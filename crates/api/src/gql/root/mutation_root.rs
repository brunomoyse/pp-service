use async_graphql::MergedObject;

use crate::gql::domains::attendance::AttendanceMutation;
use crate::gql::domains::auth::AuthMutation;
use crate::gql::domains::entries::EntryMutation;
use crate::gql::domains::identity::IdentityMutation;
use crate::gql::domains::notes::NotesMutation;
use crate::gql::domains::pro::ProMutation;
use crate::gql::domains::registrations::RegistrationMutation;
use crate::gql::domains::results::ResultMutation;
use crate::gql::domains::seating::SeatingMutation;
use crate::gql::domains::templates::TemplateMutation;
use crate::gql::domains::tournaments::{TournamentClockMutation, TournamentMutation};
use crate::gql::domains::users::UserMutation;

#[derive(MergedObject, Default)]
pub struct MutationRoot(
    AttendanceMutation,
    AuthMutation,
    EntryMutation,
    IdentityMutation,
    NotesMutation,
    ProMutation,
    RegistrationMutation,
    ResultMutation,
    SeatingMutation,
    TemplateMutation,
    TournamentClockMutation,
    TournamentMutation,
    UserMutation,
);
