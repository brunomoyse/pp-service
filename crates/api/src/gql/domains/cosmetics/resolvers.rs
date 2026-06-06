use async_graphql::{Context, Object, Result, ID};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::auth::permissions::{require_admin, require_club_manager};
use crate::features::{require_feature, Feature};
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::models::UserCosmeticRow;
use infra::repos::cosmetics;

use super::types::CosmeticItem;

fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

async fn owned_map(db: &infra::db::Db, user_id: Uuid) -> Result<HashMap<Uuid, UserCosmeticRow>> {
    let rows = cosmetics::list_owned(db, user_id).await?;
    Ok(rows.into_iter().map(|r| (r.cosmetic_item_id, r)).collect())
}

#[derive(Default)]
pub struct CosmeticsQuery;

#[Object]
impl CosmeticsQuery {
    /// The cosmetics catalog, each flagged with the current user's ownership.
    async fn cosmetic_catalog(
        &self,
        ctx: &Context<'_>,
        kind: Option<String>,
    ) -> Result<Vec<CosmeticItem>> {
        require_feature(Feature::Cosmetics)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;

        let owned = owned_map(&state.db, user_id).await?;
        let rows = cosmetics::list_catalog(&state.db, kind.as_deref()).await?;
        Ok(rows
            .into_iter()
            .map(|r| CosmeticItem::from_row(r, &owned))
            .collect())
    }

    /// Cosmetics the current user owns.
    async fn my_cosmetics(&self, ctx: &Context<'_>) -> Result<Vec<CosmeticItem>> {
        require_feature(Feature::Cosmetics)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;

        let owned = owned_map(&state.db, user_id).await?;
        // Resolve each owned id to its catalog row.
        let mut out = Vec::with_capacity(owned.len());
        for item_id in owned.keys() {
            if let Some(row) = cosmetics::get_item(&state.db, *item_id).await? {
                out.push(CosmeticItem::from_row(row, &owned));
            }
        }
        out.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
        Ok(out)
    }
}

#[derive(Default)]
pub struct CosmeticsMutation;

#[Object]
impl CosmeticsMutation {
    /// Buy a cosmetic with euros. Deterministic: the buyer receives exactly the
    /// named item at its listed price (G1). Payment capture is handled by the
    /// euro payment provider out-of-band; this records the euro purchase + grant.
    async fn purchase_cosmetic(
        &self,
        ctx: &Context<'_>,
        cosmetic_item_id: ID,
    ) -> Result<CosmeticItem> {
        require_feature(Feature::Cosmetics)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let item_id = Uuid::parse_str(cosmetic_item_id.as_str()).gql_err("Invalid item ID")?;

        let item = cosmetics::get_item(&state.db, item_id)
            .await?
            .filter(|i| i.active)
            .ok_or_else(|| async_graphql::Error::new("Cosmetic not available"))?;

        cosmetics::purchase(&state.db, user_id, &item).await?;
        let owned = owned_map(&state.db, user_id).await?;
        Ok(CosmeticItem::from_row(item, &owned))
    }

    /// Equip an owned cosmetic, replacing any other of the same kind.
    async fn equip_cosmetic(
        &self,
        ctx: &Context<'_>,
        cosmetic_item_id: ID,
    ) -> Result<CosmeticItem> {
        require_feature(Feature::Cosmetics)?;
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let item_id = Uuid::parse_str(cosmetic_item_id.as_str()).gql_err("Invalid item ID")?;

        if cosmetics::get_owned(&state.db, user_id, item_id)
            .await?
            .is_none()
        {
            return Err(async_graphql::Error::new("You do not own this cosmetic"));
        }
        let item = cosmetics::get_item(&state.db, item_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Cosmetic not found"))?;

        cosmetics::equip(&state.db, user_id, item_id, &item.kind).await?;
        let owned = owned_map(&state.db, user_id).await?;
        Ok(CosmeticItem::from_row(item, &owned))
    }

    /// Gift a cosmetic to a player (club reward credit). Club-branded items are
    /// gated to that club's managers; global items to admins.
    async fn grant_cosmetic(
        &self,
        ctx: &Context<'_>,
        cosmetic_item_id: ID,
        app_user_id: ID,
    ) -> Result<bool> {
        require_feature(Feature::Cosmetics)?;
        let state = ctx.data::<AppState>()?;
        let item_id = Uuid::parse_str(cosmetic_item_id.as_str()).gql_err("Invalid item ID")?;
        let target = Uuid::parse_str(app_user_id.as_str()).gql_err("Invalid user ID")?;

        let item = cosmetics::get_item(&state.db, item_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Cosmetic not found"))?;

        match item.club_id {
            Some(club_id) => {
                require_club_manager(ctx, club_id).await?;
            }
            None => {
                require_admin(ctx).await?;
            }
        }

        cosmetics::grant(&state.db, target, item_id, "club_gift").await?;
        Ok(true)
    }
}
