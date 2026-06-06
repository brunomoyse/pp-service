use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::jwt::Claims;
use crate::gql::error::ResultExt;
use crate::state::AppState;
use infra::repos::attendance;

use super::service;
use super::types::{AttendanceStreak, CheckInResult};

#[derive(Default)]
pub struct AttendanceQuery;

#[Object]
impl AttendanceQuery {
    /// The current user's attendance streak, or null if they've never checked in.
    async fn my_attendance_streak(&self, ctx: &Context<'_>) -> Result<Option<AttendanceStreak>> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;

        let row = attendance::get_streak(&state.db, user_id).await?;
        Ok(row.map(AttendanceStreak::from))
    }
}

#[derive(Default)]
pub struct AttendanceMutation;

#[Object]
impl AttendanceMutation {
    /// Record the current user's check-in for a tournament and advance their
    /// attendance streak. Idempotent per tournament.
    async fn record_check_in(&self, ctx: &Context<'_>, tournament_id: ID) -> Result<CheckInResult> {
        let state = ctx.data::<AppState>()?;
        let claims = ctx.data::<Claims>()?;
        let user_id = Uuid::parse_str(&claims.sub).gql_err("Invalid user ID")?;
        let tid = Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let outcome = service::record_check_in(&state.db, user_id, tid).await?;
        Ok(CheckInResult {
            streak: AttendanceStreak::from(outcome.streak),
            already_checked_in: outcome.already_checked_in,
            freeze_used: outcome.freeze_used,
            is_comeback: outcome.is_comeback,
            is_new_longest: outcome.is_new_longest,
        })
    }
}
