use async_graphql::{ComplexObject, Context, SimpleObject, ID};
use chrono::Utc;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;

/// A shared flame stays "alive" only while both friends keep turning up: it dies
/// if more than this many days pass without a joint check-in night.
const FLAME_ALIVE_DAYS: i64 = 8;

/// Head-to-head record against a single opponent. Your "nemesis" is simply the
/// rivalry with the most losses.
#[derive(SimpleObject, Clone, Debug)]
pub struct Rivalry {
    pub opponent_id: ID,
    pub opponent_name: String,
    /// Tournaments both players finished.
    pub meetings: i32,
    /// Tournaments you finished above this opponent.
    pub wins: i32,
    /// Tournaments this opponent finished above you.
    pub losses: i32,
}

impl From<infra::models::RivalryRow> for Rivalry {
    fn from(r: infra::models::RivalryRow) -> Self {
        Self {
            opponent_id: r.opponent_id.into(),
            opponent_name: r.opponent_name,
            meetings: r.meetings as i32,
            wins: r.wins as i32,
            losses: r.losses as i32,
        }
    }
}

/// The mutual flame between the current user and a friend — alive only while
/// they keep checking in on the same nights.
#[derive(SimpleObject, Clone, Debug)]
pub struct MutualFlame {
    /// Distinct nights both players checked in.
    pub shared_nights: i32,
    pub last_shared: Option<chrono::NaiveDate>,
    /// Whether the flame is still burning (recent joint check-in).
    pub alive: bool,
}

/// A friend (or pending request), resolved to the other party.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Friend {
    pub friendship_id: ID,
    pub user_id: ID,
    pub name: String,
    /// `pending` or `accepted`.
    pub status: String,
    /// True when this is a request the current user received.
    pub is_incoming: bool,
    /// True when the current user may register this friend into tournaments.
    pub i_can_register_them: bool,
    /// True when this friend may register the current user into tournaments.
    pub can_register_me: bool,
}

#[ComplexObject]
impl Friend {
    /// The shared flame with this friend (zeroed for non-accepted entries).
    async fn flame(&self, ctx: &Context<'_>) -> async_graphql::Result<MutualFlame> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let me = uuid::Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;
        let other = uuid::Uuid::parse_str(self.user_id.as_str()).gql_err("Invalid user ID")?;

        let row = infra::repos::friendships::flame_between(&state.db, me, other).await?;
        let today = Utc::now().date_naive();
        let alive = row
            .last_shared
            .map(|d| (today - d).num_days() <= FLAME_ALIVE_DAYS)
            .unwrap_or(false);

        Ok(MutualFlame {
            shared_nights: row.shared_nights as i32,
            last_shared: row.last_shared,
            alive,
        })
    }
}

impl From<infra::models::FriendRow> for Friend {
    fn from(r: infra::models::FriendRow) -> Self {
        Self {
            friendship_id: r.friendship_id.into(),
            user_id: r.user_id.into(),
            name: r.name,
            status: r.status,
            is_incoming: r.is_incoming,
            i_can_register_them: r.i_can_register_them,
            can_register_me: r.can_register_me,
        }
    }
}

/// "Your Year in Poker" — a shareable annual recap. All figures derive from
/// existing tournament/check-in data; nothing new is tracked.
#[derive(SimpleObject, Clone, Debug, Default)]
pub struct YearInPoker {
    pub year: i32,
    pub tournaments: i32,
    pub buyins_cents: i32,
    pub winnings_cents: i32,
    pub net_cents: i32,
    pub itm_count: i32,
    pub best_finish: Option<i32>,
    pub check_ins: i32,
    pub longest_streak: i32,
    pub favorite_club: Option<String>,
    pub nemesis_name: Option<String>,
}
