// Barrel re-export file: all types live in domain-specific modules.
// Consumers continue to use `use crate::gql::types::X;` unchanged.

// Common types (Role, notifications, pagination)
pub use crate::gql::common::types::{
    NotificationType, PaginatedResponse, PaginationInput, Role, UserNotification,
    TITLE_REGISTRATION_CONFIRMED, TITLE_TOURNAMENT_STARTING, TITLE_WAITLISTED,
    TITLE_WAITLIST_PROMOTED,
};

// Activity log types
pub use crate::gql::domains::activity_log::types::{ActivityEventCategory, ActivityLogEntry};

// Club types
pub use crate::gql::domains::clubs::types::{Club, ClubTable};

// User types
pub use crate::gql::domains::users::types::{CreatePlayerInput, UpdatePlayerInput, User};

// Registration types
pub use crate::gql::domains::registrations::types::{
    AssignmentStrategy, CancelRegistrationInput, CancelRegistrationResponse, CheckInPlayerInput,
    CheckInResponse, PlayerRegistrationEvent, RegisterForTournamentInput, RegistrationEventType,
    RegistrationStatus, SelfCheckInInput, SelfCheckInResponse, TournamentPlayer,
    TournamentRegistration, UpdateRegistrationStatusInput,
};

// Seating types
pub use crate::gql::domains::seating::types::{
    AssignPlayerToSeatInput, AssignTableToTournamentInput, BalanceTablesInput,
    CreateTournamentTableInput, MovePlayerInput, SeatAssignment, SeatWithPlayer,
    SeatingChangeEvent, SeatingEventType, TableWithSeats, TournamentSeatingChart, TournamentTable,
    UnassignTableFromTournamentInput, UpdateStackSizeInput,
};

// Tournament types
pub use crate::gql::domains::tournaments::types::{
    ClockStatus, CreateTournamentInput, Tournament, TournamentClock, TournamentLiveStatus,
    TournamentStatus, TournamentStructure, TournamentStructureInput, UpdateTournamentInput,
    UpdateTournamentStatusInput,
};

// Auth types
pub use crate::gql::domains::auth::types::{
    AuthPayload, CreateOAuthClientInput, CreateOAuthClientResponse, OAuthCallbackInput,
    OAuthClient, OAuthUrlResponse, RequestPasswordResetInput, RequestPasswordResetResponse,
    ResetPasswordInput, ResetPasswordResponse, UserLoginInput, UserRegistrationInput,
};

// Entry types
pub use crate::gql::domains::entries::types::{
    AddTournamentEntryInput, EntryType, TournamentEntry, TournamentEntryStats,
};

// Result types
pub use crate::gql::domains::results::types::{
    CustomPayout, CustomPayoutInput, DealType, EnterTournamentResultsInput,
    EnterTournamentResultsResponse, PayoutPosition, PlayerDeal, PlayerDealInput,
    PlayerPositionInput, PlayerStatistics, PlayerStatsResponse, TournamentPayout, TournamentResult,
    UserTournamentResult,
};

// Leaderboard types
pub use crate::gql::domains::leaderboards::types::{LeaderboardEntry, LeaderboardPeriod};

// Template types
pub use crate::gql::domains::templates::types::{
    BlindStructureLevel, BlindStructureTemplate, PayoutStructureEntry, PayoutTemplate,
};
