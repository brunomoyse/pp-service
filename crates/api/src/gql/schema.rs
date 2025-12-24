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

    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(state) // AppState is Clone; available in resolvers via ctx.data::<AppState>()
        .data(club_loader)
        .data(user_loader)
        .data(tournament_loader)
        .finish()
}
