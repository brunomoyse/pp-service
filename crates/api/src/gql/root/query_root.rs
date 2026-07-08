use async_graphql::MergedObject;

use crate::gql::domains::achievements::AchievementQuery;
use crate::gql::domains::activity_log::ActivityLogQuery;
use crate::gql::domains::analytics::AnalyticsQuery;
use crate::gql::domains::announcements::AnnouncementQuery;
use crate::gql::domains::attendance::AttendanceQuery;
use crate::gql::domains::auth::AuthQuery;
use crate::gql::domains::clubs::ClubQuery;
use crate::gql::domains::drinks::DrinksQuery;
use crate::gql::domains::entries::EntryQuery;
use crate::gql::domains::identity::IdentityQuery;
use crate::gql::domains::leaderboard_configs::LeaderboardConfigQuery;
use crate::gql::domains::leaderboards::LeaderboardQuery;
use crate::gql::domains::notes::NotesQuery;
use crate::gql::domains::predictions::PredictionsQuery;
use crate::gql::domains::registrations::RegistrationQuery;
use crate::gql::domains::results::ResultQuery;
use crate::gql::domains::scouting::ScoutingQuery;
use crate::gql::domains::seasons::SeasonsQuery;
use crate::gql::domains::seating::SeatingQuery;
use crate::gql::domains::series::SeriesQuery;
use crate::gql::domains::social::SocialQuery;
use crate::gql::domains::templates::TemplateQuery;
use crate::gql::domains::tournaments::{TournamentClockQuery, TournamentQuery};
use crate::gql::domains::users::UserQuery;

#[derive(MergedObject, Default)]
pub struct QueryRoot(
    AchievementQuery,
    ActivityLogQuery,
    AnalyticsQuery,
    AnnouncementQuery,
    AttendanceQuery,
    AuthQuery,
    ClubQuery,
    DrinksQuery,
    EntryQuery,
    IdentityQuery,
    LeaderboardConfigQuery,
    LeaderboardQuery,
    NotesQuery,
    PredictionsQuery,
    RegistrationQuery,
    ResultQuery,
    ScoutingQuery,
    SeasonsQuery,
    SeatingQuery,
    SeriesQuery,
    SocialQuery,
    TemplateQuery,
    TournamentClockQuery,
    TournamentQuery,
    UserQuery,
);
