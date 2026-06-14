use async_graphql::MergedObject;

use crate::gql::domains::announcements::AnnouncementMutation;
use crate::gql::domains::attendance::AttendanceMutation;
use crate::gql::domains::auth::AuthMutation;
use crate::gql::domains::clubs::ClubMutation;
use crate::gql::domains::cosmetics::CosmeticsMutation;
use crate::gql::domains::devices::DeviceMutation;
use crate::gql::domains::drinks::DrinksMutation;
use crate::gql::domains::entries::EntryMutation;
use crate::gql::domains::identity::IdentityMutation;
use crate::gql::domains::leaderboard_configs::LeaderboardConfigMutation;
use crate::gql::domains::notes::NotesMutation;
use crate::gql::domains::predictions::PredictionsMutation;
use crate::gql::domains::pro::ProMutation;
use crate::gql::domains::registrations::RegistrationMutation;
use crate::gql::domains::results::ResultMutation;
use crate::gql::domains::scouting::ScoutingMutation;
use crate::gql::domains::seasons::SeasonsMutation;
use crate::gql::domains::seating::SeatingMutation;
use crate::gql::domains::series::SeriesMutation;
use crate::gql::domains::social::SocialMutation;
use crate::gql::domains::templates::TemplateMutation;
use crate::gql::domains::tournaments::{TournamentClockMutation, TournamentMutation};
use crate::gql::domains::users::UserMutation;

#[derive(MergedObject, Default)]
pub struct MutationRoot(
    AnnouncementMutation,
    AttendanceMutation,
    AuthMutation,
    ClubMutation,
    CosmeticsMutation,
    DeviceMutation,
    DrinksMutation,
    EntryMutation,
    IdentityMutation,
    LeaderboardConfigMutation,
    NotesMutation,
    PredictionsMutation,
    ProMutation,
    RegistrationMutation,
    ResultMutation,
    ScoutingMutation,
    SeasonsMutation,
    SeatingMutation,
    SeriesMutation,
    SocialMutation,
    TemplateMutation,
    TournamentClockMutation,
    TournamentMutation,
    UserMutation,
);
