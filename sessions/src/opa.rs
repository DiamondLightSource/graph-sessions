use axum_extra::headers::{authorization::Bearer, Authorization};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use url::Url;

/// Parametrers required by OPA to make the policy decision
#[derive(Debug, Serialize)]
pub struct OpaInput<P: Serialize> {
    /// The access Json Web Token (JWT) associated with the request
    pub token: Option<String>,
    /// Additional parameters required by OPA
    pub parameters: P,
}

impl<P: Serialize> OpaInput<P> {
    /// Create an [`OpaInput`] from an [`async_graphql::Context`] and some requisite parameters
    pub fn new(ctx: &async_graphql::Context, parameters: P) -> Result<Self, async_graphql::Error> {
        Ok(Self {
            token: ctx
                .data::<Option<Authorization<Bearer>>>()?
                .as_ref()
                .map(|header| header.token().to_string()),
            parameters,
        })
    }
}

/// The policy decision made by opa
#[derive(Debug, Deserialize)]
pub struct Decision {
    /// Whether the operation should be permitted
    pub allow: bool,
}

/// An Open Policy Agent client
#[derive(Debug)]
pub struct OpaClient {
    /// A configured [`reqwest::Client`]
    client: reqwest::Client,
    /// The OPA endpoint to make requests against
    endpoint: Url,
}

impl OpaClient {
    /// Creates a new [`OpaClient`] bound to the provided endpoint [`Url`]
    pub fn new(endpoint: Url) -> Self {
        info!("Setting up OPA client at {endpoint}");
        Self {
            client: reqwest::Client::new(),
            endpoint,
        }
    }

    /// Queries OPA with the [`OpaInput`] and returns the [`Decision`]
    #[instrument(skip(self, input))]
    async fn query<P: Serialize>(&self, input: OpaInput<P>) -> Result<Decision, reqwest::Error> {
        let mut request = self
            .client
            .post(self.endpoint.clone())
            .json(&input)
            .build()?;

        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(
                &tracing::Span::current().context(),
                &mut opentelemetry_http::HeaderInjector(request.headers_mut()),
            )
        });

        self.client.execute(request).await?.json().await
    }

    /// Queries OPA with the [`OpaInput`] and returns a [`Result`]
    pub async fn decide<P: Serialize>(&self, input: OpaInput<P>) -> Result<(), anyhow::Error> {
        self.query(input)
            .await?
            .allow
            .then_some(())
            .ok_or(anyhow::anyhow!("Access denied"))
    }
}
