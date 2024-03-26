#![forbid(unsafe_code)]
#![doc=include_str!("../../README.md")]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

/// Metadata about the crate, courtesy of [`built`]
mod built_info;
/// GraphQL resolvers
mod graphql;
/// Open Policy Agent helpers
mod opa;
/// An [`axum::handler::Handler`] for GraphQL
mod route_handlers;

use crate::{
    graphql::{root_schema_builder, RootSchema},
    opa::OpaClient,
    route_handlers::GraphQLHandler,
};
use async_graphql::{http::GraphiQLSource, SDLExportOptions};
use axum::{response::Html, routing::get, Router};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use clap::Parser;
use opentelemetry_otlp::WithExportConfig;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr, TransactionError};
use std::{
    fs::File,
    io::Write,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
    time::Duration,
};
use tokio::net::TcpListener;
use tracing::{info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

/// A service providing Beamline Session data from ISPyB
#[derive(Debug, Parser)]
#[command(author, version, about, long_about=None)]
#[allow(clippy::large_enum_variant)]
enum Cli {
    /// Starts a webserver serving the GraphQL API
    Serve(ServeArgs),
    /// Produces the GraphQL schema
    Schema(SchemaArgs),
}

/// Arguments for serving the GraphQL API
#[derive(Debug, Parser)]
struct ServeArgs {
    /// The port to which this application should bind
    #[arg(short, long, env = "PORT", default_value_t = 80)]
    port: u16,
    /// The URL of the ISPyB instance which should be connected to
    #[arg(long, env = "DATABASE_URL")]
    database_url: Url,
    /// The URL of the Open Policy Agent instance used for authorization
    #[arg(long, env = "OPA_URL")]
    opa_url: Url,
    /// The [`tracing::Level`] to log at
    #[arg(long, env = "LOG_LEVEL", default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,
    /// The URL of the OpenTelemetry collector to send traces to
    #[arg(long, env = "OTEL_COLLECTOR_URL")]
    otel_collector_url: Option<Url>,
}

/// Arguments for produces the GraphQL schema
#[derive(Debug, Parser)]
struct SchemaArgs {
    /// The path to write the schema to, if not set the schema will be printed to stdout
    #[arg(short, long)]
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args = Cli::parse();

    match args {
        Cli::Serve(args) => {
            setup_telemetry(args.log_level, args.otel_collector_url).unwrap();
            let database = setup_database(args.database_url).await.unwrap();
            let opa_client = OpaClient::new(args.opa_url);
            let schema = root_schema_builder()
                .data(database)
                .data(opa_client)
                .finish();
            let router = setup_router(schema);
            serve(router, args.port).await.unwrap();
        }
        Cli::Schema(args) => {
            let schema = root_schema_builder().finish();
            let schema_string = schema.sdl_with_options(SDLExportOptions::new().federation());
            if let Some(path) = args.path {
                let mut file = File::create(path).unwrap();
                file.write_all(schema_string.as_bytes()).unwrap();
            } else {
                println!("{}", schema_string)
            }
        }
    }
}

/// Creates a connection pool to access the database
#[instrument(skip(database_url))]
async fn setup_database(database_url: Url) -> Result<DatabaseConnection, TransactionError<DbErr>> {
    info!("Connecting to database at {database_url}");
    let connection_options = ConnectOptions::new(database_url.to_string())
        .sqlx_logging_level(tracing::log::LevelFilter::Debug)
        .to_owned();
    let connection = Database::connect(connection_options).await?;
    info!("Database connection established: {connection:?}");
    Ok(connection)
}

/// Creates an [`axum::Router`] serving GraphiQL, synchronous GraphQL and GraphQL subscriptions
fn setup_router(schema: RootSchema) -> Router {
    #[allow(clippy::missing_docs_in_private_items)]
    const GRAPHQL_ENDPOINT: &str = "/";

    Router::new()
        .route(
            GRAPHQL_ENDPOINT,
            get(Html(
                GraphiQLSource::build().endpoint(GRAPHQL_ENDPOINT).finish(),
            ))
            .post(GraphQLHandler::new(schema)),
        )
        .layer(OtelInResponseLayer)
        .layer(OtelAxumLayer::default())
}

/// Serves the endpoints on the specified port forever
async fn serve(router: Router, port: u16) -> Result<(), std::io::Error> {
    let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));
    let listener = TcpListener::bind(socket_addr).await?;
    println!("Serving API & GraphQL UI at {}", socket_addr);
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}

/// Sets up Logging & Tracing using opentelemetry if available
fn setup_telemetry(
    log_level: tracing::Level,
    otel_collector_url: Option<Url>,
) -> Result<(), anyhow::Error> {
    let level_filter = tracing_subscriber::filter::LevelFilter::from_level(log_level);
    let log_layer = tracing_subscriber::fmt::layer();
    let service_name_resource = opentelemetry_sdk::Resource::new(vec![
        opentelemetry::KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            built_info::PKG_NAME,
        ),
        opentelemetry::KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
            built_info::PKG_VERSION,
        ),
    ]);
    let (metrics_layer, tracing_layer) = if let Some(otel_collector_url) = otel_collector_url {
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::default(),
        );
        (
            Some(tracing_opentelemetry::MetricsLayer::new(
                opentelemetry_otlp::new_pipeline()
                    .metrics(opentelemetry_sdk::runtime::Tokio)
                    .with_exporter(
                        opentelemetry_otlp::new_exporter()
                            .tonic()
                            .with_endpoint(otel_collector_url.clone()),
                    )
                    .with_resource(service_name_resource.clone())
                    .with_period(Duration::from_secs(10))
                    .build()?,
            )),
            Some(
                tracing_opentelemetry::layer().with_tracer(
                    opentelemetry_otlp::new_pipeline()
                        .tracing()
                        .with_exporter(
                            opentelemetry_otlp::new_exporter()
                                .tonic()
                                .with_endpoint(otel_collector_url),
                        )
                        .with_trace_config(
                            opentelemetry_sdk::trace::config().with_resource(service_name_resource),
                        )
                        .install_batch(opentelemetry_sdk::runtime::Tokio)?,
                ),
            ),
        )
    } else {
        (None, None)
    };

    tracing_subscriber::Registry::default()
        .with(level_filter)
        .with(log_layer)
        .with(metrics_layer)
        .with(tracing_layer)
        .init();

    Ok(())
}
