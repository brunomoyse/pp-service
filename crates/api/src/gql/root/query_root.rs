use async_graphql::MergedObject;

use crate::gql::domains::achievements::AchievementQuery;
use crate::gql::domains::activity_log::ActivityLogQuery;
use crate::gql::domains::analytics::AnalyticsQuery;
use crate::gql::domains::attendance::AttendanceQuery;
use crate::gql::domains::auth::AuthQuery;
use crate::gql::domains::clubs::ClubQuery;
use crate::gql::domains::entries::EntryQuery;
use crate::gql::domains::identity::IdentityQuery;
use crate::gql::domains::leaderboards::LeaderboardQuery;
use crate::gql::domains::notes::NotesQuery;
use crate::gql::domains::pro::ProQuery;
use crate::gql::domains::registrations::RegistrationQuery;
use crate::gql::domains::results::ResultQuery;
use crate::gql::domains::seating::SeatingQuery;
use crate::gql::domains::system::SystemQuery;
use crate::gql::domains::templates::TemplateQuery;
use crate::gql::domains::tournaments::{TournamentClockQuery, TournamentQuery};
use crate::gql::domains::users::UserQuery;

#[derive(MergedObject, Default)]
pub struct QueryRoot(
    AchievementQuery,
    ActivityLogQuery,
    AnalyticsQuery,
    AttendanceQuery,
    AuthQuery,
    ClubQuery,
    EntryQuery,
    IdentityQuery,
    LeaderboardQuery,
    NotesQuery,
    ProQuery,
    RegistrationQuery,
    ResultQuery,
    SeatingQuery,
    SystemQuery,
    TemplateQuery,
    TournamentClockQuery,
    TournamentQuery,
    UserQuery,
);
