use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Object, Schema, SchemaBuilder, SimpleObject,
};
use chrono::{DateTime, Utc};
use models::{bl_session, proposal};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, JoinType, QueryFilter, QuerySelect,
};

/// The GraphQL schema exposed by the service
pub type RootSchema = Schema<RootQuery, EmptyMutation, EmptySubscription>;

/// A schema builder for the service
pub fn root_schema_builder() -> SchemaBuilder<RootQuery, EmptyMutation, EmptySubscription> {
    Schema::build(RootQuery, EmptyMutation, EmptySubscription)
}

/// A Beamline Session
#[derive(Debug, SimpleObject)]
struct Session {
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
    /// Retrieves a Beamline Session
    async fn session(
        &self,
        ctx: &Context<'_>,
        proposal: u32,
        visit: u32,
    ) -> Result<Option<Session>, async_graphql::Error> {
        let database = ctx.data::<DatabaseConnection>()?;
        Ok(bl_session::Entity::find()
            .join_rev(
                JoinType::InnerJoin,
                proposal::Entity::has_many(bl_session::Entity)
                    .from(proposal::Column::ProposalId)
                    .to(bl_session::Column::ProposalId)
                    .into(),
            )
            .filter(
                Condition::all()
                    .add(bl_session::Column::VisitNumber.eq(visit))
                    .add(proposal::Column::ProposalNumber.eq(proposal)),
            )
            .one(database)
            .await?
            .map(Session::from))
    }
}
