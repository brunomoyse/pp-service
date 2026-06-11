use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::auth::permissions::require_club_manager;
use crate::gql::error::ResultExt;
use crate::gql::subscriptions::publish_user_notification;
use crate::gql::types::{
    NotificationType, Tournament, UserNotification, TITLE_QUALIFIED_FOR_DAY_2,
};
use crate::state::AppState;
use infra::repos::tournament_clock::TournamentStructureLevel;
use infra::repos::tournaments::CreateTournamentData;
use infra::repos::{
    club_players, flight_qualifications, tournament_clock, tournament_registrations,
    tournament_series, tournaments,
};

use super::types::{CloseFlightInput, CreateTournamentSeriesInput, FlightInput, TournamentSeries};

#[derive(Default)]
pub struct SeriesQuery;

#[Object]
impl SeriesQuery {
    /// A single multi-day series (with its flights + qualifications). Managers only.
    async fn tournament_series(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<TournamentSeries>> {
        let state = ctx.data::<AppState>()?;
        let series_uuid = Uuid::parse_str(id.as_str()).gql_err("Invalid series ID")?;
        let Some(row) = tournament_series::get_by_id(&state.db, series_uuid).await? else {
            return Ok(None);
        };
        require_club_manager(ctx, row.club_id).await?;
        Ok(Some(TournamentSeries::from(row)))
    }

    /// All series for a club, most recent first. Managers only.
    async fn tournament_series_list(
        &self,
        ctx: &Context<'_>,
        club_id: ID,
    ) -> Result<Vec<TournamentSeries>> {
        let club_uuid = Uuid::parse_str(club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_uuid).await?;

        let state = ctx.data::<AppState>()?;
        let rows = tournament_series::list_by_club(&state.db, club_uuid).await?;
        Ok(rows.into_iter().map(TournamentSeries::from).collect())
    }
}

#[derive(Default)]
pub struct SeriesMutation;

#[Object]
impl SeriesMutation {
    /// Create a multi-day series: the event plus one tournament per Day-1 flight
    /// and the final day, all sharing the same buy-in / blind structure.
    async fn create_tournament_series(
        &self,
        ctx: &Context<'_>,
        input: CreateTournamentSeriesInput,
    ) -> Result<TournamentSeries> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;
        require_club_manager(ctx, club_id).await?;

        if input.flights.is_empty() {
            return Err(async_graphql::Error::new(
                "A series needs at least one flight",
            ));
        }

        // Resolve the shared blind structure once (template or custom), applied
        // to every flight + the final day.
        let levels = resolve_levels(state, &input.template_id, &input.structure).await?;

        let series = tournament_series::create(
            &state.db,
            club_id,
            input.title.clone(),
            input.best_stack_forward.unwrap_or(true),
        )
        .await
        .gql_err("Failed to create series")?;

        // Create each flight, then the final day.
        let mut to_create: Vec<(FlightInput, bool)> =
            input.flights.into_iter().map(|f| (f, false)).collect();
        to_create.push((input.final_day, true));

        for (flight, is_final_day) in to_create {
            let data = CreateTournamentData {
                club_id,
                name: format!("{} — {}", input.title, flight.label),
                description: None,
                start_time: flight.start_time,
                end_time: None,
                buy_in_cents: input.buy_in_cents,
                rake_cents: input.rake_cents,
                seat_cap: input.seat_cap,
                early_bird_bonus_chips: None,
                level_two_bonus_chips: None,
                voucher_value_cents: None,
                rebuy_max: None,
                addon_chips: None,
                addon_price_cents: None,
                late_registration_level: input.late_registration_level,
                bounty_type: None,
                bounty_amount_cents: None,
                leaderboard_config_id: None,
                series_id: Some(series.id),
                flight_label: Some(flight.label),
                is_final_day,
            };
            let row = tournaments::create(&state.db, data)
                .await
                .gql_err("Failed to create flight")?;

            for level in &levels {
                tournament_clock::add_structure(&state.db, row.id, level.clone())
                    .await
                    .gql_err("Failed to add structure level")?;
            }
        }

        Ok(TournamentSeries::from(series))
    }

    /// Close a Day-1 flight: record its survivors as qualifications (best stack
    /// forward) and mark the flight finished. Survivors with an app account are
    /// notified they're through to Day 2.
    async fn close_flight(&self, ctx: &Context<'_>, input: CloseFlightInput) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        let flight = tournaments::get_by_id(&state.db, tournament_id)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
        require_club_manager(ctx, flight.club_id).await?;

        let Some(series_id) = flight.series_id else {
            return Err(async_graphql::Error::new(
                "Tournament is not part of a series",
            ));
        };
        if flight.is_final_day {
            return Err(async_graphql::Error::new(
                "Cannot close the final day as a flight",
            ));
        }

        let final_day_id = tournament_series::final_day_id(&state.db, series_id)
            .await
            .gql_err("Database operation failed")?;

        for survivor in &input.survivors {
            let club_player_id = Uuid::parse_str(survivor.club_player_id.as_str())
                .gql_err("Invalid club player ID")?;
            flight_qualifications::record(
                &state.db,
                series_id,
                club_player_id,
                tournament_id,
                survivor.chip_count,
            )
            .await
            .gql_err("Failed to record qualification")?;
            flight_qualifications::refresh_best(&state.db, series_id, club_player_id)
                .await
                .gql_err("Failed to refresh best stack")?;

            // Notify the survivor (in-app + push), if they have an account.
            if let Ok(Some(cp)) = club_players::get_by_id(&state.db, club_player_id).await {
                if let Some(user_id) = cp.app_user_id {
                    let chip_count = survivor.chip_count;
                    publish_user_notification(UserNotification {
                        id: ID::from(Uuid::new_v4().to_string()),
                        user_id: ID::from(user_id.to_string()),
                        notification_type: NotificationType::QualifiedForDay2,
                        title: TITLE_QUALIFIED_FOR_DAY_2.to_string(),
                        message: format!("You've qualified for Day 2 with {chip_count} chips."),
                        tournament_id: final_day_id.map(|id| ID::from(id.to_string())),
                        created_at: chrono::Utc::now(),
                    });
                    if let Some(fd) = final_day_id {
                        let db = state.db.clone();
                        tokio::spawn(async move {
                            crate::services::push_service::send_qualified_for_day2(
                                &db, user_id, fd, chip_count,
                            )
                            .await;
                        });
                    }
                }
            }
        }

        let updated = tournaments::update_live_status(
            &state.db,
            tournament_id,
            infra::repos::tournaments::TournamentLiveStatus::Finished,
        )
        .await
        .gql_err("Failed to finish flight")?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        Ok(Tournament::from(updated))
    }

    /// Seed (or refresh) the final day's registrations from the series'
    /// qualifiers: each best-stack survivor gets a CHECKED_IN registration with
    /// their carried-over stack. Idempotent.
    async fn open_day_two(&self, ctx: &Context<'_>, series_id: ID) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let series_uuid = Uuid::parse_str(series_id.as_str()).gql_err("Invalid series ID")?;

        let series = tournament_series::get_by_id(&state.db, series_uuid)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Series not found"))?;
        require_club_manager(ctx, series.club_id).await?;

        let final_day_id = tournament_series::final_day_id(&state.db, series_uuid)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Series has no final day"))?;

        let qualifiers = flight_qualifications::list_best_by_series(&state.db, series_uuid)
            .await
            .gql_err("Failed to load qualifiers")?;

        for q in qualifiers {
            tournament_registrations::upsert_checked_in_with_stack(
                &state.db,
                final_day_id,
                q.club_player_id,
                q.chip_count,
            )
            .await
            .gql_err("Failed to seed Day 2 registration")?;
        }

        let final_day = tournaments::get_by_id(&state.db, final_day_id)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Final day not found"))?;
        Ok(Tournament::from(final_day))
    }
}

/// Resolve the shared blind structure for a series from either a template id or
/// an explicit custom structure (template takes precedence). Empty when neither.
async fn resolve_levels(
    state: &AppState,
    template_id: &Option<ID>,
    structure: &Option<Vec<crate::gql::domains::tournaments::types::TournamentStructureInput>>,
) -> Result<Vec<TournamentStructureLevel>> {
    if let Some(template_id) = template_id {
        let template_uuid = Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;
        let template = infra::repos::blind_structure_templates::get_by_id(&state.db, template_uuid)
            .await
            .gql_err("Failed to fetch template")?
            .ok_or_else(|| async_graphql::Error::new("Template not found"))?;
        let levels: Vec<crate::gql::domains::templates::types::BlindStructureLevel> =
            serde_json::from_value(template.levels).gql_err("Invalid template levels format")?;
        return Ok(levels
            .into_iter()
            .map(|l| TournamentStructureLevel {
                level_number: l.level_number,
                small_blind: l.small_blind,
                big_blind: l.big_blind,
                ante: l.ante,
                duration_minutes: l.duration_minutes,
                is_break: l.is_break,
                break_duration_minutes: l.break_duration_minutes,
            })
            .collect());
    }

    if let Some(custom) = structure {
        return Ok(custom
            .iter()
            .map(|l| TournamentStructureLevel {
                level_number: l.level_number,
                small_blind: l.small_blind,
                big_blind: l.big_blind,
                ante: l.ante,
                duration_minutes: l.duration_minutes,
                is_break: l.is_break,
                break_duration_minutes: l.break_duration_minutes,
            })
            .collect());
    }

    Ok(Vec::new())
}
