use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Object, Schema, SchemaBuilder, SimpleObject,
};
use chrono::{DateTime, Utc};
use models::bl_session;
use sea_orm::{DatabaseConnection, EntityTrait};

/// The GraphQL schema exposed by the service
pub type RootSchema = Schema<RootQuery, EmptyMutation, EmptySubscription>;

/// A schema builder for the service
pub fn root_schema_builder() -> SchemaBuilder<RootQuery, EmptyMutation, EmptySubscription> {
    Schema::build(RootQuery, EmptyMutation, EmptySubscription)
}

/// A Beamline Session
#[derive(Debug, SimpleObject)]
struct Session {
    /// An opaque unique identifier for the session
    session_id: u32,
    /// The number of session within the Proposal
    visit_number: Option<u32>,
    /// The date and time at which the Session began
    start: Option<DateTime<Utc>>,
    /// The date and time at which the Session ended
    end: Option<DateTime<Utc>>,
}

impl From<bl_session::Model> for Session {
    fn from(value: bl_session::Model) -> Self {
        Self {
            session_id: value.session_id,
            visit_number: value.visit_number,
            start: value.start_date.map(|date| date.and_utc()),
            end: value.end_date.map(|date| date.and_utc()),
        }
    }
}

/// The root query of the service
#[derive(Debug, Clone, Default)]
pub struct RootQuery;

#[Object]
impl RootQuery {
    /// Retrieves all Beamline Sessions
    async fn sessions(&self, ctx: &Context<'_>) -> Result<Vec<Session>, async_graphql::Error> {
        let database = ctx.data::<DatabaseConnection>()?;
        Ok(bl_session::Entity::find()
            .all(database)
            .await?
            .into_iter()
            .map(Session::from)
            .collect())
    }

    /// Retrieves a Beamline Session
    async fn session(
        &self,
        ctx: &Context<'_>,
        session_id: u32,
    ) -> Result<Option<Session>, async_graphql::Error> {
        let database = ctx.data::<DatabaseConnection>()?;
        Ok(bl_session::Entity::find_by_id(session_id)
            .one(database)
            .await?
            .map(Session::from))
    }
}
