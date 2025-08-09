use async_graphql::Schema;

use crate::state::AppState;
use super::{QueryRoot, MutationRoot, SubscriptionRoot};

/// Build the GraphQL schema and inject shared state (AppState) into the context.
pub fn build_schema(state: AppState) -> Schema<QueryRoot, MutationRoot, SubscriptionRoot> {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(state) // AppState is Clone; available in resolvers via ctx.data::<AppState>()
        .finish()
}