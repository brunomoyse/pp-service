use async_graphql::MergedObject;

use crate::gql::domains::auth::AuthQuery;
use crate::gql::domains::clubs::ClubQuery;
use crate::gql::domains::entries::EntryQuery;
use crate::gql::domains::leaderboards::LeaderboardQuery;
use crate::gql::domains::registrations::RegistrationQuery;
use crate::gql::domains::results::ResultQuery;
use crate::gql::domains::seating::SeatingQuery;
use crate::gql::domains::templates::TemplateQuery;
use crate::gql::domains::tournaments::{TournamentClockQuery, TournamentQuery};
use crate::gql::domains::users::UserQuery;

#[derive(MergedObject, Default)]
pub struct QueryRoot(
    AuthQuery,
    ClubQuery,
    EntryQuery,
    LeaderboardQuery,
    RegistrationQuery,
    ResultQuery,
    SeatingQuery,
    TemplateQuery,
    TournamentClockQuery,
    TournamentQuery,
    UserQuery,
);
