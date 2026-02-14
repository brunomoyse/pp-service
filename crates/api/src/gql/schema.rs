use std::env;

use async_graphql::dataloader::DataLoader;
use async_graphql::Schema;

use super::loaders::{ClubLoader, TournamentLoader, UserLoader};
use super::{MutationRoot, QueryRoot, SubscriptionRoot};
use crate::state::AppState;

/// Build the GraphQL schema and inject shared state (AppState) into the context.
pub fn build_schema(state: AppState) -> Schema<QueryRoot, MutationRoot, SubscriptionRoot> {
    let club_loader = DataLoader::new(ClubLoader::new(state.db.clone()), tokio::spawn);
    let user_loader = DataLoader::new(UserLoader::new(state.db.clone()), tokio::spawn);
    let tournament_loader = DataLoader::new(TournamentLoader::new(state.db.clone()), tokio::spawn);

    // Introspection is enabled by default in development
    let introspection_enabled = env::var("GQL_INTROSPECTION")
        .map(|v| v == "true")
        .unwrap_or(true);

    // Query depth limit (default: 30, suitable for introspection)
    let depth_limit = env::var("GQL_QUERY_DEPTH_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(30);

    // Query complexity limit (default: 2000, suitable for introspection)
    let complexity_limit = env::var("GQL_QUERY_COMPLEXITY_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2000);

    let mut builder = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        SubscriptionRoot,
    )
    .data(state) // AppState is Clone; available in resolvers via ctx.data::<AppState>()
    .data(club_loader)
    .data(user_loader)
    .data(tournament_loader)
    .limit_depth(depth_limit)
    .limit_complexity(complexity_limit);

    if !introspection_enabled {
        builder = builder.disable_introspection();
    }

    builder.finish()
}
