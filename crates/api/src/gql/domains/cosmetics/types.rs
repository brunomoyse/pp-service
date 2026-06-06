use async_graphql::{SimpleObject, ID};
use std::collections::HashMap;

use infra::models::{CosmeticItemRow, UserCosmeticRow};

/// A named, previewable, fixed-price cosmetic. There is no random acquisition
/// path — buying yields exactly this item (G1).
#[derive(SimpleObject, Clone, Debug)]
pub struct CosmeticItem {
    pub id: ID,
    pub code: String,
    /// One of `card_back`, `avatar_frame`, `theme`, `badge`.
    pub kind: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub preview_ref: String,
    pub club_id: Option<ID>,
    /// Whether the current user owns this item.
    pub owned: bool,
    /// Whether the current user has it equipped.
    pub equipped: bool,
}

impl CosmeticItem {
    /// Build a catalog entry, stamping ownership from the user's owned map.
    pub fn from_row(row: CosmeticItemRow, owned: &HashMap<uuid::Uuid, UserCosmeticRow>) -> Self {
        let mine = owned.get(&row.id);
        Self {
            id: row.id.into(),
            code: row.code,
            kind: row.kind,
            name: row.name,
            description: row.description,
            price_cents: row.price_cents,
            preview_ref: row.preview_ref,
            club_id: row.club_id.map(Into::into),
            owned: mine.is_some(),
            equipped: mine.map(|m| m.equipped).unwrap_or(false),
        }
    }
}
