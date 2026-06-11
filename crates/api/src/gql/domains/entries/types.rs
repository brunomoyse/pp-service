use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum EntryType {
    Initial,
    Rebuy,
    ReEntry,
    Addon,
    /// Mandatory drink voucher — collected from the player, excluded from the prize pool.
    Voucher,
    /// Chip-only grant (e.g. the level-2 early-bird bonus); excluded from the prize pool.
    Bonus,
}

impl From<String> for EntryType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "initial" => EntryType::Initial,
            "rebuy" => EntryType::Rebuy,
            "re_entry" => EntryType::ReEntry,
            "addon" => EntryType::Addon,
            "voucher" => EntryType::Voucher,
            "bonus" => EntryType::Bonus,
            _ => EntryType::Initial,
        }
    }
}

impl From<EntryType> for String {
    fn from(e: EntryType) -> Self {
        match e {
            EntryType::Initial => "initial".to_string(),
            EntryType::Rebuy => "rebuy".to_string(),
            EntryType::ReEntry => "re_entry".to_string(),
            EntryType::Addon => "addon".to_string(),
            EntryType::Voucher => "voucher".to_string(),
            EntryType::Bonus => "bonus".to_string(),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum PaymentMethod {
    Cash,
    Card,
    BankTransfer,
    Voucher,
    Comp,
    Other,
}

impl From<String> for PaymentMethod {
    fn from(s: String) -> Self {
        match s.as_str() {
            "cash" => PaymentMethod::Cash,
            "card" => PaymentMethod::Card,
            "bank_transfer" => PaymentMethod::BankTransfer,
            "voucher" => PaymentMethod::Voucher,
            "comp" => PaymentMethod::Comp,
            "other" => PaymentMethod::Other,
            _ => PaymentMethod::Cash,
        }
    }
}

impl From<PaymentMethod> for String {
    fn from(m: PaymentMethod) -> Self {
        match m {
            PaymentMethod::Cash => "cash",
            PaymentMethod::Card => "card",
            PaymentMethod::BankTransfer => "bank_transfer",
            PaymentMethod::Voucher => "voucher",
            PaymentMethod::Comp => "comp",
            PaymentMethod::Other => "other",
        }
        .to_string()
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentEntry {
    pub id: ID,
    pub tournament_id: ID,
    /// The app user, when this player has an account. Null for account-less players.
    pub user_id: Option<ID>,
    /// The club roster identity — always present.
    pub club_player_id: ID,
    pub entry_type: EntryType,
    pub amount_cents: i32,
    pub chips_received: Option<i32>,
    pub recorded_by: Option<ID>,
    pub notes: Option<String>,
    pub payment_method: PaymentMethod,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<infra::models::TournamentEntryRow> for TournamentEntry {
    fn from(row: infra::models::TournamentEntryRow) -> Self {
        Self {
            id: row.id.into(),
            tournament_id: row.tournament_id.into(),
            user_id: row.user_id.map(Into::into),
            club_player_id: row.club_player_id.into(),
            entry_type: EntryType::from(row.entry_type),
            amount_cents: row.amount_cents,
            chips_received: row.chips_received,
            recorded_by: row.recorded_by.map(|id| id.into()),
            notes: row.notes,
            payment_method: PaymentMethod::from(row.payment_method),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TournamentEntryStats {
    pub tournament_id: ID,
    pub total_entries: i32,
    pub total_amount_cents: i32,
    pub unique_players: i32,
    pub initial_count: i32,
    pub rebuy_count: i32,
    pub re_entry_count: i32,
    pub addon_count: i32,
    pub total_rake_cents: i32,
    pub total_chips: i64,
    pub players_remaining: i32,
}

#[derive(InputObject)]
pub struct AddTournamentEntryInput {
    pub tournament_id: ID,
    pub user_id: ID,
    pub entry_type: EntryType,
    pub amount_cents: Option<i32>,
    pub chips_received: Option<i32>,
    pub notes: Option<String>,
    /// How the player paid; defaults to CASH when omitted.
    pub payment_method: Option<PaymentMethod>,
}

/// One cell of the cash report: money taken in for a (method, type) pair.
#[derive(SimpleObject, Clone)]
pub struct CashReportLine {
    pub payment_method: PaymentMethod,
    pub entry_type: EntryType,
    pub amount_cents: i32,
    pub count: i32,
}

/// End-of-night cash report for a single tournament. The manager pivots
/// `lines` into a method-by-type matrix; the totals reconcile the drawer.
#[derive(SimpleObject, Clone)]
pub struct TournamentCashReport {
    pub tournament_id: ID,
    pub lines: Vec<CashReportLine>,
    /// Sum of every entry's amount across all methods (gross collected).
    pub total_collected_cents: i32,
    pub total_rake_cents: i32,
    pub prize_pool_cents: i32,
    pub entry_count: i32,
}
