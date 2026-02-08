use async_graphql::MergedObject;

use crate::gql::domains::auth::AuthMutation;
use crate::gql::domains::entries::EntryMutation;
use crate::gql::domains::registrations::RegistrationMutation;
use crate::gql::domains::results::ResultMutation;
use crate::gql::domains::seating::SeatingMutation;
use crate::gql::domains::tournaments::TournamentClockMutation;
use crate::gql::domains::users::UserMutation;

#[derive(MergedObject, Default)]
pub struct MutationRoot(
    AuthMutation,
    EntryMutation,
    RegistrationMutation,
    ResultMutation,
    SeatingMutation,
    TournamentClockMutation,
    UserMutation,
);
