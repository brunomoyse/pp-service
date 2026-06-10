use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::permissions::require_club_manager;
use crate::auth::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;

use super::service;
use super::types::{
    ActivatePrintedCardInput, ActivatePrintedCardPayload, BarStation, ClaimCardInput,
    ClaimCardPayload, CreateBarStationInput, DrinkCard, DrinkWallet, GenerateDrinkCardsInput,
    GenerateDrinkCardsPayload, RedeemDrinkInput, RedeemDrinkPayload, TopUpWalletInput,
    TopUpWalletPayload,
};

use infra::repos::{bar_stations, club_players, drink_wallets, users};

/// Resolve the authenticated user id from the JWT claims.
fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx
        .data::<Claims>()
        .map_err(|_| async_graphql::Error::new("You must be logged in to perform this action"))?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

/// A display name for a user, used when a player claims a bearer card.
fn display_name_from_user(u: &infra::models::UserRow) -> String {
    let last = u.last_name.clone().unwrap_or_default();
    let name = format!("{} {}", u.first_name, last);
    let name = name.trim().to_string();
    if name.is_empty() {
        u.username.clone().unwrap_or_else(|| u.email.clone())
    } else {
        name
    }
}

#[derive(Default)]
pub struct DrinksQuery;

#[Object]
impl DrinksQuery {
    /// A wallet's cached balance and recent ledger. Visible to the wallet's owner or
    /// to a manager of the wallet's club.
    async fn drink_wallet(&self, ctx: &Context<'_>, wallet_id: ID) -> Result<DrinkWallet> {
        let state = ctx.data::<AppState>()?;
        let wallet_id = Uuid::parse_str(wallet_id.as_str()).gql_err("Invalid wallet ID")?;

        let wallet = drink_wallets::get_by_id(&state.db, wallet_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Wallet not found"))?;

        // Owner access: the caller owns the roster person this wallet is bound to.
        let user_id = current_user_id(ctx)?;
        let is_owner = match wallet.club_player_id {
            Some(rp_id) => club_players::get_by_id(&state.db, rp_id)
                .await?
                .and_then(|rp| rp.app_user_id)
                .is_some_and(|owner| owner == user_id),
            None => false,
        };

        if !is_owner {
            // Otherwise require club-manager authority over the wallet's club.
            require_club_manager(ctx, wallet.club_id).await?;
        }

        Ok(wallet.into())
    }
}

#[derive(Default)]
pub struct DrinksMutation;

#[Object]
impl DrinksMutation {
    /// Create a bar station (a point of redemption) for a club. Manager/admin only.
    async fn create_bar_station(
        &self,
        ctx: &Context<'_>,
        input: CreateBarStationInput,
    ) -> Result<BarStation> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_id).await?;

        let station = bar_stations::create(&state.db, club_id, &input.name).await?;
        Ok(station.into())
    }

    /// Generate blank printed cards. Each token is returned exactly once. Manager/admin only.
    async fn generate_drink_cards(
        &self,
        ctx: &Context<'_>,
        input: GenerateDrinkCardsInput,
    ) -> Result<GenerateDrinkCardsPayload> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_id).await?;

        let cards = service::generate_printed_cards(&state.db, input.count)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(GenerateDrinkCardsPayload {
            cards: cards
                .into_iter()
                .map(|c| DrinkCard {
                    credential_id: c.credential_id.into(),
                    token: c.token,
                })
                .collect(),
        })
    }

    /// Activate a printed card into a wallet (named or bearer), optionally seeding a
    /// balance. Manager/admin only.
    async fn activate_printed_card(
        &self,
        ctx: &Context<'_>,
        input: ActivatePrintedCardInput,
    ) -> Result<ActivatePrintedCardPayload> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        let manager = require_club_manager(ctx, club_id).await?;
        let operator_user_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid user ID")?;

        let outcome = service::activate_printed_card(
            &state.db,
            service::ActivateParams {
                raw_token: input.credential_token,
                club_id,
                display_name: input.display_name,
                initial_top_up: input.initial_top_up,
                expires_at: input.expires_at,
                operator_user_id,
            },
        )
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(ActivatePrintedCardPayload {
            wallet: outcome.wallet.into(),
        })
    }

    /// Add credits to a wallet. Manager/admin of the wallet's club only.
    async fn top_up_wallet(
        &self,
        ctx: &Context<'_>,
        input: TopUpWalletInput,
    ) -> Result<TopUpWalletPayload> {
        let state = ctx.data::<AppState>()?;
        let wallet_id = Uuid::parse_str(input.wallet_id.as_str()).gql_err("Invalid wallet ID")?;

        let wallet = drink_wallets::get_by_id(&state.db, wallet_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Wallet not found"))?;
        let manager = require_club_manager(ctx, wallet.club_id).await?;
        let operator_user_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid user ID")?;

        let tournament_id = match input.tournament_id {
            Some(id) => Some(Uuid::parse_str(id.as_str()).gql_err("Invalid tournament ID")?),
            None => None,
        };

        let outcome = service::top_up_wallet(
            &state.db,
            service::TopUpParams {
                wallet_id,
                amount: input.amount,
                tournament_id,
                expires_at: input.expires_at,
                operator_user_id,
            },
        )
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(TopUpWalletPayload {
            wallet_id: outcome.wallet_id.into(),
            balance: outcome.balance,
            ledger_entry: outcome.ledger_entry.into(),
        })
    }

    /// Redeem one drink at the bar. Requires a manager/admin operating the given bar
    /// station (a trusted station identity, never player self-service). Idempotent on
    /// `idempotencyKey` and safe under concurrent scans.
    async fn redeem_drink(
        &self,
        ctx: &Context<'_>,
        input: RedeemDrinkInput,
    ) -> Result<RedeemDrinkPayload> {
        let state = ctx.data::<AppState>()?;
        let bar_station_id =
            Uuid::parse_str(input.bar_station_id.as_str()).gql_err("Invalid bar station ID")?;

        // Authorize against the station's club, then the service confirms the wallet
        // belongs to that same club.
        let station = bar_stations::get_by_id(&state.db, bar_station_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Bar station not found"))?;
        let manager = require_club_manager(ctx, station.club_id).await?;
        let operator_user_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid user ID")?;

        let outcome = service::redeem_drink(
            &state.db,
            service::RedeemParams {
                raw_token: input.credential_token,
                bar_station_id,
                idempotency_key: input.idempotency_key,
                drink_type: input.drink_type,
                operator_user_id,
            },
        )
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(RedeemDrinkPayload {
            wallet_id: outcome.wallet_id.into(),
            balance: outcome.balance,
            redemption: outcome.redemption.into(),
            deduped: outcome.deduped,
        })
    }

    /// Claim a card to your own account. Binds an owner to the card's existing wallet
    /// without moving any balance. Any authenticated player may claim.
    async fn claim_card(
        &self,
        ctx: &Context<'_>,
        input: ClaimCardInput,
    ) -> Result<ClaimCardPayload> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;

        let user = users::get_by_id(&state.db, user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;
        let display_name = display_name_from_user(&user);

        let outcome = service::claim_card(
            &state.db,
            service::ClaimParams {
                raw_token: input.credential_token,
                app_user_id: user_id,
                display_name,
            },
        )
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(ClaimCardPayload {
            wallet: outcome.wallet.into(),
            message: outcome.message,
        })
    }
}
