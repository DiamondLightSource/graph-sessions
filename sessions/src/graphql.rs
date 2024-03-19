use crate::opa::{OpaClient, OpaInput};
use async_graphql::{
    ComplexObject, Context, EmptyMutation, EmptySubscription, Object, Schema, SchemaBuilder,
    SimpleObject,
};
use chrono::{DateTime, Utc};
use models::{bl_session, proposal};
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use tracing::instrument;

/// The GraphQL schema exposed by the service
pub type RootSchema = Schema<Query, EmptyMutation, EmptySubscription>;

/// A schema builder for the service
pub fn root_schema_builder() -> SchemaBuilder<Query, EmptyMutation, EmptySubscription> {
    Schema::build(Query, EmptyMutation, EmptySubscription).enable_federation()
}

/// A Beamline Session
#[derive(Debug, SimpleObject)]
#[graphql(complex, unresolvable = "id")]
struct Session {
    /// The underlying database model
    #[graphql(skip)]
    session: bl_session::Model,
    /// The proposal information
    proposal: Option<Proposal>,
}

#[ComplexObject]
impl Session {
    async fn id(&self, _ctx: &Context<'_>) -> u32 {
        self.session.session_id
    }

    async fn visit(&self, _ctx: &Context<'_>) -> u32 {
        self.session.visit_number.unwrap_or_default()
    }

    async fn start(&self, _ctx: &Context<'_>) -> Option<DateTime<Utc>> {
        self.session.start_date.map(|date| date.and_utc())
    }

    async fn end(&self, _ctx: &Context<'_>) -> Option<DateTime<Utc>> {
        self.session.end_date.map(|date| date.and_utc())
    }
}

/// An Experimental Proposal, containing numerous sessions
#[derive(Debug)]
struct Proposal(proposal::Model);

#[Object]
impl Proposal {
    async fn code(&self, _ctx: &Context<'_>) -> &Option<String> {
        &self.0.proposal_code
    }

    /// A unique number identifying the Proposal
    async fn number(&self, _ctx: &Context<'_>) -> Result<Option<u32>, async_graphql::Error> {
        Ok(self
            .0
            .proposal_number
            .as_ref()
            .map(|num| num.parse())
            .transpose()?)
    }
}

/// The root query of the service
#[derive(Debug, Clone, Default)]
pub struct Query;

/// Parameters required to
#[derive(Debug, Serialize)]
struct OpaSessionParameters {
    /// The proposal of the session being requested
    proposal: u32,
    /// The visit number of the session being requested
    visit: u32,
}

#[Object]
impl Query {
    /// Retrieves a Beamline Session
    #[instrument(name = "query_session", skip(ctx))]
    async fn session(
        &self,
        ctx: &Context<'_>,
        proposal: u32,
        visit: u32,
    ) -> Result<Option<Session>, async_graphql::Error> {
        let database = ctx.data::<DatabaseConnection>()?;
        ctx.data::<OpaClient>()?
            .decide(OpaInput::new(
                ctx,
                OpaSessionParameters { proposal, visit },
            )?)
            .await?;
        Ok(bl_session::Entity::find()
            .find_also_related(proposal::Entity)
            .filter(
                Condition::all()
                    .add(bl_session::Column::VisitNumber.eq(visit))
                    .add(proposal::Column::ProposalNumber.eq(proposal)),
            )
            .one(database)
            .await?
            .map(|(session, proposal)| Session {
                session,
                proposal: proposal.map(Proposal),
            }))
    }
}
