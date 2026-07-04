use async_graphql::dataloader::Loader;
use infra::{
    db::Db, models::ClubPlayerRow, models::ClubRow, models::DrinkLedgerEntryRow,
    models::DrinkWalletRow, models::TournamentRow, models::UserRow,
};
use std::{collections::HashMap, future::Future, sync::Arc};
use uuid::Uuid;

// ClubPlayerLoader - batch load roster entries (the club-scoped player
// identity) by id. Used to render an account-less player's display name.
#[derive(Clone)]
pub struct ClubPlayerLoader {
    pool: Db,
}

impl ClubPlayerLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for ClubPlayerLoader {
    type Value = ClubPlayerRow;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            let rows: Vec<ClubPlayerRow> = sqlx::query_as::<_, ClubPlayerRow>(
                r#"
                SELECT id, club_id, display_name, first_name, last_name, app_user_id, is_active, created_at, updated_at
                FROM club_player
                WHERE id = ANY($1::uuid[])
                "#,
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(Arc::new)?;

            Ok(rows.into_iter().map(|r| (r.id, r)).collect())
        }
    }
}

#[derive(Clone)]
pub struct ClubLoader {
    pool: Db,
}

impl ClubLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for ClubLoader {
    type Value = ClubRow;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            let rows: Vec<ClubRow> = sqlx::query_as::<_, ClubRow>(
                r#"
                SELECT id, name, city, postal_code, province, country, address, vat_number, needs_review, plan, subscription_status, subscription_expires_at, created_at, updated_at
                FROM clubs
                WHERE id = ANY($1::uuid[])
                "#,
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(Arc::new)?;

            Ok(rows.into_iter().map(|r| (r.id, r)).collect())
        }
    }
}

// UserLoader - batch load users by ID
#[derive(Clone)]
pub struct UserLoader {
    pool: Db,
}

impl UserLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for UserLoader {
    type Value = UserRow;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            let rows: Vec<UserRow> = sqlx::query_as::<_, UserRow>(
                r#"
                SELECT id, email, username, first_name, last_name, phone,
                       is_active, role, locale, created_at, updated_at
                FROM users
                WHERE id = ANY($1::uuid[])
                "#,
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(Arc::new)?;

            Ok(rows.into_iter().map(|r| (r.id, r)).collect())
        }
    }
}

// DrinkWalletLoader - batch load drink wallets by ID
#[derive(Clone)]
pub struct DrinkWalletLoader {
    pool: Db,
}

impl DrinkWalletLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for DrinkWalletLoader {
    type Value = DrinkWalletRow;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            let rows: Vec<DrinkWalletRow> = sqlx::query_as::<_, DrinkWalletRow>(
                r#"
                SELECT id, club_player_id, club_id, balance, created_at, updated_at
                FROM drink_wallet
                WHERE id = ANY($1::uuid[])
                "#,
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(Arc::new)?;

            Ok(rows.into_iter().map(|r| (r.id, r)).collect())
        }
    }
}

// DrinkLedgerLoader - batch load the recent ledger entries (newest first, capped at
// 20 per wallet) for a set of wallets, to render wallet histories without N+1.
#[derive(Clone)]
pub struct DrinkLedgerLoader {
    pool: Db,
}

impl DrinkLedgerLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for DrinkLedgerLoader {
    type Value = Vec<DrinkLedgerEntryRow>;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            // Window per wallet, keep the 20 newest entries each.
            let rows: Vec<DrinkLedgerEntryRow> = sqlx::query_as::<_, DrinkLedgerEntryRow>(
                r#"
                SELECT id, wallet_id, delta, reason, tournament_id, expires_at,
                       redemption_id, source_ledger_entry_id, transfer_id, created_by, created_at
                FROM (
                    SELECT *, ROW_NUMBER() OVER (
                        PARTITION BY wallet_id ORDER BY created_at DESC, id DESC
                    ) AS rn
                    FROM drink_ledger_entry
                    WHERE wallet_id = ANY($1::uuid[])
                ) ranked
                WHERE rn <= 20
                ORDER BY created_at DESC, id DESC
                "#,
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(Arc::new)?;

            let mut map: HashMap<Uuid, Vec<DrinkLedgerEntryRow>> = HashMap::new();
            for row in rows {
                map.entry(row.wallet_id).or_default().push(row);
            }
            Ok(map)
        }
    }
}

// TournamentLoader - batch load tournaments by ID
#[derive(Clone)]
pub struct TournamentLoader {
    pool: Db,
}

impl TournamentLoader {
    pub fn new(pool: Db) -> Self {
        Self { pool }
    }
}

impl Loader<Uuid> for TournamentLoader {
    type Value = TournamentRow;
    type Error = Arc<sqlx::Error>;

    fn load(
        &self,
        keys: &[Uuid],
    ) -> impl Future<Output = std::result::Result<HashMap<Uuid, Self::Value>, Self::Error>> + Send
    {
        let pool = self.pool.clone();
        let ids: Vec<Uuid> = keys.to_vec();

        async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }

            let rows: Vec<TournamentRow> = sqlx::query_as::<_, TournamentRow>(
                r#"
                SELECT id, club_id, name, description, start_time, end_time,
                       buy_in_cents, rake_cents, seat_cap, starting_stack, live_status, early_bird_bonus_chips,
                       level_two_bonus_chips, voucher_value_cents, rebuy_max, addon_chips,
                       addon_price_cents, late_registration_level, bounty_type, bounty_amount_cents, leaderboard_config_id, series_id, flight_label, is_final_day, created_at, updated_at
                FROM tournaments
                WHERE id = ANY($1::uuid[])
                "#,
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(Arc::new)?;

            Ok(rows.into_iter().map(|r| (r.id, r)).collect())
        }
    }
}
