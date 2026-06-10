// Barrel re-export file: all types live in domain-specific modules.
// Consumers continue to use `use crate::gql::types::X;` unchanged.

// Common types (Role, notifications, pagination)
pub use crate::gql::common::types::{
    NotificationType, PaginatedResponse, PaginationInput, Role, UserNotification,
    TITLE_PLAYER_ELIMINATED, TITLE_PLAYER_MOVED, TITLE_REGISTRATION_CONFIRMED, TITLE_SEAT_ASSIGNED,
    TITLE_TOURNAMENT_STARTING, TITLE_WAITLISTED, TITLE_WAITLIST_PROMOTED,
};

// Activity log types
pub use crate::gql::domains::activity_log::types::{ActivityEventCategory, ActivityLogEntry};

// Club types
pub use crate::gql::domains::clubs::types::{
    Club, ClubTable, CompanyLookup, OnboardClubInput, OnboardClubPayload,
};

// Identity / roster types
pub use crate::gql::domains::identity::types::{
    ClaimClubPlayerInput, ClubPlayer, CreateClubPlayerInput,
};

// Notes types
pub use crate::gql::domains::notes::types::{
    AddPlayerNoteTagInput, AddShowdownObservationInput, FieldPlayerNote, NoteTagKind, PlayerNote,
    PlayerNoteTag, PlayerStyle, ShowdownObservation, UpsertPlayerNoteInput,
};

// System types
pub use crate::gql::domains::system::types::FeatureFlags;

// Pro entitlement types
pub use crate::gql::domains::pro::types::{GrantProEntitlementInput, ProEntitlement};

// Analytics types
pub use crate::gql::domains::analytics::types::{
    BuyInBreakdown, ClubBreakdown, PnlPoint, ProAnalytics,
};

// Attendance / streak types
pub use crate::gql::domains::attendance::types::{AttendanceStreak, CheckInResult};

// Season / season-pass / quest types
pub use crate::gql::domains::seasons::types::{
    CreateSeasonInput, HallOfFameEntry, QuestProgress, Season, SeasonPass,
};

// Social types
pub use crate::gql::domains::social::types::{Friend, MutualFlame, Rivalry, YearInPoker};

// Cosmetics (euro economy) types
pub use crate::gql::domains::cosmetics::types::CosmeticItem;

// Predictions (Prediction-Points economy) types
pub use crate::gql::domains::predictions::types::{PredictionBalance, PredictionEntry};

// Scouting / privacy (public-stats) types
pub use crate::gql::domains::scouting::types::{
    PrivacySettings, ScoutingMatch, ScoutingProfile, ScoutingQuota,
};

// User types
pub use crate::gql::domains::users::types::{CreatePlayerInput, UpdatePlayerInput, User};

// Registration types
pub use crate::gql::domains::registrations::types::{
    AssignmentStrategy, CancelRegistrationInput, CancelRegistrationResponse, CheckInPlayerInput,
    CheckInResponse, PlayerRegistrationEvent, RegisterForTournamentInput,
    RegisterRosterPlayerInput, RegistrationEventType, RegistrationStatus, SelfCheckInInput,
    SelfCheckInResponse, TournamentPlayer, TournamentRegistration, UpdateRegistrationStatusInput,
};

// Seating types
pub use crate::gql::domains::seating::types::{
    AssignPlayerToSeatInput, AssignTableToTournamentInput, AssignTablesToTournamentInput,
    BalanceTablesInput, BulkAssignTableEntry, CreateTournamentTableInput, MovePlayerInput,
    SeatAssignment, SeatWithPlayer, SeatingChangeEvent, SeatingEventType, TableWithSeats,
    TournamentSeatingChart, TournamentTable, UnassignTableFromTournamentInput, UnseatedPlayer,
    UpdateStackSizeInput,
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
    BlindStructureLevel, BlindStructureLevelInput, BlindStructureTemplate,
    CreateBlindStructureTemplateInput, CreatePayoutTemplateInput, PayoutStructureEntry,
    PayoutStructureEntryInput, PayoutTemplate, UpdateBlindStructureTemplateInput,
    UpdatePayoutTemplateInput,
};
