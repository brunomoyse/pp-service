use async_graphql::{Context, Object, Result, ID};
use chrono::{Datelike, Utc};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::{attendance, friendships, rivalries, wrapped};

use super::types::{Friend, Rivalry, YearInPoker};

fn current_user_id(ctx: &Context<'_>) -> Result<Uuid> {
    let claims = ctx.data::<Claims>()?;
    Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")
}

#[derive(Default)]
pub struct SocialQuery;

#[Object]
impl SocialQuery {
    /// The current user's head-to-head records, most-played opponents first.
    /// The nemesis is the entry with the most losses.
    async fn my_rivalries(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 10)] limit: i32,
    ) -> Result<Vec<Rivalry>> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let capped = limit.clamp(1, 50) as i64;
        let rows = rivalries::for_user(&state.db, user_id, capped).await?;
        Ok(rows.into_iter().map(Rivalry::from).collect())
    }

    /// The current user's accepted friends.
    async fn my_friends(&self, ctx: &Context<'_>) -> Result<Vec<Friend>> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let rows = friendships::list_friends(&state.db, user_id).await?;
        Ok(rows.into_iter().map(Friend::from).collect())
    }

    /// Pending friend requests the current user has received.
    async fn incoming_friend_requests(&self, ctx: &Context<'_>) -> Result<Vec<Friend>> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let rows = friendships::list_incoming(&state.db, user_id).await?;
        Ok(rows.into_iter().map(Friend::from).collect())
    }

    /// Pending friend requests the current user has sent (awaiting the other party).
    async fn outgoing_friend_requests(&self, ctx: &Context<'_>) -> Result<Vec<Friend>> {
        let state = ctx.data::<AppState>()?;
        let user_id = current_user_id(ctx)?;
        let rows = friendships::list_outgoing(&state.db, user_id).await?;
        Ok(rows.into_iter().map(Friend::from).collect())
    }

    /// "Your Year in Poker" — a shareable annual recap (defaults to this year).
    async fn my_year_in_poker(&self, ctx: &Context<'_>, year: Option<i32>) -> Result<YearInPoker> {
        let state = ctx.data::<AppState>()?;
        let db = &state.db;
        let user_id = current_user_id(ctx)?;
        let year = year.unwrap_or_else(|| Utc::now().year());

        let stats = wrapped::stats_for_year(db, user_id, year).await?;
        let check_ins = wrapped::check_ins_for_year(db, user_id, year).await?;
        let favorite = wrapped::favorite_club_for_year(db, user_id, year).await?;
        let longest_streak = attendance::get_streak(db, user_id)
            .await?
            .map(|s| s.longest_streak)
            .unwrap_or(0);
        // Lifetime nemesis: the opponent who has beaten the player most.
        let nemesis = rivalries::for_user(db, user_id, 50)
            .await?
            .into_iter()
            .filter(|r| r.losses > 0)
            .max_by_key(|r| (r.losses, r.meetings))
            .map(|r| r.opponent_name);

        let winnings = stats.winnings_cents;
        let buyins = stats.buyins_cents;
        Ok(YearInPoker {
            year,
            tournaments: stats.tournaments as i32,
            buyins_cents: buyins as i32,
            winnings_cents: winnings as i32,
            net_cents: (winnings - buyins) as i32,
            itm_count: stats.itm_count as i32,
            best_finish: stats.best_finish,
            check_ins: check_ins as i32,
            longest_streak,
            favorite_club: favorite.map(|f| f.club_name),
            nemesis_name: nemesis,
        })
    }
}

#[derive(Default)]
pub struct SocialMutation;

#[Object]
impl SocialMutation {
    /// Send a friend request to another player.
    async fn send_friend_request(&self, ctx: &Context<'_>, user_id: ID) -> Result<Friend> {
        let state = ctx.data::<AppState>()?;
        let me = current_user_id(ctx)?;
        let other = Uuid::parse_str(user_id.as_str()).gql_err("Invalid user ID")?;

        if me == other {
            return Err(async_graphql::Error::new("You cannot befriend yourself"));
        }
        if friendships::get_between(&state.db, me, other)
            .await?
            .is_some()
        {
            return Err(async_graphql::Error::new("A friendship already exists"));
        }

        let target = infra::repos::users::get_by_id(&state.db, other)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        let row = friendships::create_request(&state.db, me, other).await?;
        Ok(Friend {
            friendship_id: row.id.into(),
            user_id: other.into(),
            name: target.username.unwrap_or(target.first_name),
            status: row.status,
            is_incoming: false,
            i_can_register_them: false,
            can_register_me: false,
        })
    }

    /// Accept a pending friend request. Only the addressee may accept.
    async fn accept_friend_request(&self, ctx: &Context<'_>, friendship_id: ID) -> Result<Friend> {
        let state = ctx.data::<AppState>()?;
        let me = current_user_id(ctx)?;
        let fid = Uuid::parse_str(friendship_id.as_str()).gql_err("Invalid friendship ID")?;

        let row = friendships::accept(&state.db, fid, me)
            .await?
            .ok_or_else(|| async_graphql::Error::new("No pending request to accept"))?;

        let other_id = if row.requester_id == me {
            row.addressee_id
        } else {
            row.requester_id
        };
        let other = infra::repos::users::get_by_id(&state.db, other_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(Friend {
            friendship_id: row.id.into(),
            user_id: other_id.into(),
            name: other.username.unwrap_or(other.first_name),
            status: row.status,
            is_incoming: false,
            i_can_register_them: false,
            can_register_me: false,
        })
    }

    /// Remove a friend or decline a request. Either party may do so.
    async fn remove_friend(&self, ctx: &Context<'_>, friendship_id: ID) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let me = current_user_id(ctx)?;
        let fid = Uuid::parse_str(friendship_id.as_str()).gql_err("Invalid friendship ID")?;
        Ok(friendships::remove(&state.db, fid, me).await?)
    }

    /// Allow (or disallow) a friend to register the current user into tournaments.
    /// Sets the permission for the caller's own direction of the friendship.
    async fn set_friend_registration_permission(
        &self,
        ctx: &Context<'_>,
        friendship_id: ID,
        allow: bool,
    ) -> Result<bool> {
        let state = ctx.data::<AppState>()?;
        let me = current_user_id(ctx)?;
        let fid = Uuid::parse_str(friendship_id.as_str()).gql_err("Invalid friendship ID")?;

        let updated = friendships::set_registration_permission(&state.db, fid, me, allow)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Friendship not found"))?;
        Ok(updated.status == "accepted")
    }

    /// Register a friend into a tournament on their behalf. Requires an accepted
    /// friendship where the friend has granted the caller permission.
    async fn register_friend_for_tournament(
        &self,
        ctx: &Context<'_>,
        friend_user_id: ID,
        tournament_id: ID,
    ) -> Result<crate::gql::types::TournamentRegistration> {
        let state = ctx.data::<AppState>()?;
        let me = current_user_id(ctx)?;
        let friend_id = Uuid::parse_str(friend_user_id.as_str()).gql_err("Invalid user ID")?;
        let tournament_uuid =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        if !friendships::can_register(&state.db, me, friend_id).await? {
            return Err(async_graphql::Error::new(
                "This friend has not allowed you to register them",
            ));
        }

        // Reuse the registration service path. The friend has an app account, so
        // register by user_id; the link trigger resolves their roster identity.
        let mut tx = state.db.begin().await?;
        let create_data = infra::repos::tournament_registrations::CreateTournamentRegistration {
            tournament_id: tournament_uuid,
            user_id: Some(friend_id),
            registered_player_id: None,
            notes: Some("Registered by a friend".to_string()),
            status: None,
        };
        let row = infra::repos::tournament_registrations::create(&mut *tx, create_data).await?;
        tx.commit().await?;

        Ok(row.into())
    }
}
