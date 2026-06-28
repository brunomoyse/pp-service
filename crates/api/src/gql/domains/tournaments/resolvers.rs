use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::permissions::{
    is_free_plan, require_club_manager, viewer_is_admin, viewer_manages_club,
};
use crate::gql::common::helpers::tournament_hidden_from_viewer;
use crate::gql::error::ResultExt;
use crate::gql::types::{PaginatedResponse, PaginationInput, Tournament, TournamentStatus};
use crate::state::AppState;
use infra::models::TournamentRow;
use infra::repos::tournament_clock::TournamentStructureLevel;
use infra::repos::tournaments::{
    self, CreateTournamentData, TournamentFilter, TournamentLiveStatus, UpdateTournamentData,
};

use crate::gql::domains::seating::types::{SeatingChangeEvent, SeatingEventType};
use crate::gql::subscriptions::publish_seating_event;

use super::recurrence::{occurrence_starts, MAX_OCCURRENCES};
use super::types::{CreateTournamentInput, UpdateTournamentInput, UpdateTournamentStatusInput};

/// Create a single tournament occurrence inside an existing transaction: insert
/// the row, copy the resolved blind structure (if any), and — only when
/// `link_default_tables` is set — auto-link the club's default table set.
///
/// Recurring runs pass `link_default_tables = true` for the first occurrence
/// only: physical tables are time-shared and the conflict guard treats any
/// non-finished tournament as holding its tables, so reserving the same tables
/// for every future occurrence would both fail and be semantically wrong. Later
/// occurrences are created without tables; the manager links them as each event
/// approaches.
async fn create_one(
    conn: &mut sqlx::PgConnection,
    data: CreateTournamentData,
    structure: Option<&[TournamentStructureLevel]>,
    link_default_tables: bool,
) -> Result<TournamentRow> {
    let club_id = data.club_id;
    let tournament_row = tournaments::create(&mut *conn, data)
        .await
        .gql_err("Failed to create tournament")?;

    if let Some(levels) = structure {
        for level in levels {
            infra::repos::tournament_clock::add_structure(
                &mut *conn,
                tournament_row.id,
                level.clone(),
            )
            .await
            .gql_err("Failed to add structure level")?;
        }
    }

    // Auto-link the club's default table set. Tables already booked by another
    // live tournament are skipped (no double-booking). Best-effort: a
    // table-linking hiccup must not fail tournament creation.
    if link_default_tables {
        if let Ok(default_tables) =
            infra::repos::club_tables::list_default_active_by_club(&mut *conn, club_id).await
        {
            if !default_tables.is_empty() {
                let ids: Vec<Uuid> = default_tables.iter().map(|t| t.id).collect();
                let conflicting: std::collections::HashSet<Uuid> =
                    infra::repos::club_tables::active_table_conflicts(
                        &mut *conn,
                        &ids,
                        tournament_row.id,
                    )
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| c.club_table_id)
                    .collect();
                for table in default_tables {
                    if conflicting.contains(&table.id) {
                        continue;
                    }
                    let _ = infra::repos::club_tables::assign_to_tournament(
                        &mut *conn,
                        tournament_row.id,
                        table.id,
                        None,
                    )
                    .await;
                }
            }
        }
    }

    Ok(tournament_row)
}

#[derive(Default)]
pub struct TournamentQuery;

#[Object]
impl TournamentQuery {
    /// Get tournaments with optional filtering and pagination
    async fn tournaments(
        &self,
        ctx: &Context<'_>,
        club_id: Option<Uuid>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        status: Option<TournamentStatus>,
        pagination: Option<PaginationInput>,
    ) -> Result<PaginatedResponse<Tournament>> {
        let state = ctx.data::<AppState>()?;

        // Hide free ("Home Game") clubs from the player app / public. A club-
        // scoped request from that club's own manager (or an admin) still sees
        // them; global discovery only excludes them unless the viewer is admin.
        let exclude_free_clubs = match club_id {
            Some(cid) => !viewer_manages_club(ctx, cid).await,
            None => !viewer_is_admin(ctx),
        };

        let filter = TournamentFilter {
            club_id,
            from,
            to,
            status: status.map(|s| s.into()),
            exclude_free_clubs,
        };

        let page_params = pagination.unwrap_or(PaginationInput {
            limit: Some(50),
            offset: Some(0),
        });
        let limit_offset = page_params.to_limit_offset();

        // Fetch tournaments and total count in parallel
        let (rows, total_count) = tokio::try_join!(
            tournaments::list(&state.db, filter.clone(), Some(limit_offset)),
            tournaments::count(&state.db, filter)
        )
        .gql_err("Database operation failed")?;

        let items: Vec<Tournament> = rows.into_iter().map(Tournament::from).collect();
        let page_size = items.len() as i32;
        let offset = limit_offset.offset as i32;
        let has_next_page = (offset + page_size) < total_count as i32;

        Ok(PaginatedResponse {
            items,
            total_count: total_count as i32,
            page_size,
            offset,
            has_next_page,
        })
    }

    /// Get a single tournament by ID
    async fn tournament(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Tournament>> {
        let state = ctx.data::<AppState>()?;

        // A free-club tournament is invisible to the player app / public — only
        // its own managers and admins can open it directly.
        if tournament_hidden_from_viewer(ctx, id).await? {
            return Ok(None);
        }

        let row = tournaments::get_by_id(&state.db, id)
            .await
            .gql_err("Database operation failed")?;

        Ok(row.map(Tournament::from))
    }
}

#[derive(Default)]
pub struct TournamentMutation;

#[Object]
impl TournamentMutation {
    /// Create a new tournament
    async fn create_tournament(
        &self,
        ctx: &Context<'_>,
        input: CreateTournamentInput,
    ) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let club_id = Uuid::parse_str(input.club_id.as_str()).gql_err("Invalid club ID")?;

        // Check permissions
        let _user = require_club_manager(ctx, club_id).await?;

        // Free ("Home Game") tier: one-off tournaments only, and just one live
        // at a time. Recurring scheduling and concurrency are Club features.
        if is_free_plan(ctx, club_id).await? {
            if input.recurrence_frequency.is_some() {
                return Err(async_graphql::Error::new(
                    "Recurring tournaments require the Club plan. Upgrade to schedule a series.",
                ));
            }
            let active: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM tournaments WHERE club_id = $1 AND live_status <> 'finished'",
            )
            .bind(club_id)
            .fetch_one(&state.db)
            .await
            .gql_err("Database operation failed")?;
            if active > 0 {
                return Err(async_graphql::Error::new(
                    "The Home Game (free) plan allows 1 active tournament at a time. Finish the current one or upgrade to Club.",
                ));
            }
        }

        let leaderboard_config_id = input
            .leaderboard_config_id
            .as_ref()
            .map(|id| Uuid::parse_str(id.as_str()))
            .transpose()
            .gql_err("Invalid league ID")?;

        // Resolve the blind structure once (from a template or the custom
        // levels) so every occurrence gets the same structure.
        let structure: Option<Vec<TournamentStructureLevel>> =
            if let Some(template_id) = input.template_id.as_ref() {
                let template_uuid =
                    Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;
                let template =
                    infra::repos::blind_structure_templates::get_by_id(&state.db, template_uuid)
                        .await
                        .gql_err("Failed to fetch template")?
                        .ok_or_else(|| async_graphql::Error::new("Template not found"))?;
                let levels: Vec<crate::gql::domains::templates::types::BlindStructureLevel> =
                    serde_json::from_value(template.levels)
                        .gql_err("Invalid template levels format")?;
                Some(
                    levels
                        .into_iter()
                        .map(|level| TournamentStructureLevel {
                            level_number: level.level_number,
                            small_blind: level.small_blind,
                            big_blind: level.big_blind,
                            ante: level.ante,
                            duration_minutes: level.duration_minutes,
                            is_break: level.is_break,
                            break_duration_minutes: level.break_duration_minutes,
                        })
                        .collect(),
                )
            } else {
                input.structure.map(|custom| {
                    custom
                        .into_iter()
                        .map(|level| TournamentStructureLevel {
                            level_number: level.level_number,
                            small_blind: level.small_blind,
                            big_blind: level.big_blind,
                            ante: level.ante,
                            duration_minutes: level.duration_minutes,
                            is_break: level.is_break,
                            break_duration_minutes: level.break_duration_minutes,
                        })
                        .collect()
                })
            };

        // Compute the occurrence start times. Without recurrence this is just
        // the single requested start; with it, expand to the bounded run.
        let starts = match input.recurrence_frequency {
            None => vec![input.start_time],
            Some(freq) => {
                let end = input.recurrence_end_date.ok_or_else(|| {
                    async_graphql::Error::new(
                        "recurrenceEndDate is required when recurrenceFrequency is set",
                    )
                })?;
                occurrence_starts(input.start_time, end, freq, MAX_OCCURRENCES)
                    .map_err(async_graphql::Error::new)?
            }
        };
        // Preserve each occurrence's duration relative to its own start.
        let duration = input.end_time.map(|end| end - input.start_time);

        // Create all occurrences atomically: a mid-run failure rolls back the
        // whole series rather than leaving a partial run behind.
        let mut tx = state
            .db
            .begin()
            .await
            .gql_err("Failed to start transaction")?;
        let mut first: Option<TournamentRow> = None;
        for (i, start) in starts.iter().enumerate() {
            let data = CreateTournamentData {
                club_id,
                name: input.name.clone(),
                description: input.description.clone(),
                start_time: *start,
                end_time: duration.map(|d| *start + d),
                buy_in_cents: input.buy_in_cents,
                rake_cents: input.rake_cents,
                seat_cap: input.seat_cap,
                starting_stack: input.starting_stack,
                early_bird_bonus_chips: input.early_bird_bonus_chips,
                level_two_bonus_chips: input.level_two_bonus_chips,
                voucher_value_cents: input.voucher_value_cents,
                rebuy_max: input.rebuy_max,
                addon_chips: input.addon_chips,
                addon_price_cents: input.addon_price_cents,
                late_registration_level: input.late_registration_level,
                bounty_type: input.bounty_type.map(String::from),
                bounty_amount_cents: input.bounty_amount_cents,
                leaderboard_config_id,
                // Standalone tournaments are not part of a series; series flights are
                // created via the `createTournamentSeries` mutation.
                series_id: None,
                flight_label: None,
                is_final_day: false,
            };

            // Only the first occurrence reserves the club's default tables
            // (see `create_one`).
            let row = create_one(&mut tx, data, structure.as_deref(), i == 0).await?;
            if first.is_none() {
                first = Some(row);
            }
        }
        tx.commit().await.gql_err("Failed to create tournament")?;

        // Return the first occurrence; the client refetches the list to see the
        // full run.
        Ok(Tournament::from(first.expect("at least one occurrence")))
    }

    /// Update an existing tournament
    async fn update_tournament(
        &self,
        ctx: &Context<'_>,
        input: UpdateTournamentInput,
    ) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let tournament_id = Uuid::parse_str(input.id.as_str()).gql_err("Invalid tournament ID")?;

        // Get tournament to check club_id
        let existing = tournaments::get_by_id(&state.db, tournament_id)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Check permissions
        let _user = require_club_manager(ctx, existing.club_id).await?;

        // Update tournament data
        let data = UpdateTournamentData {
            name: input.name,
            description: input.description,
            start_time: input.start_time,
            end_time: input.end_time,
            buy_in_cents: input.buy_in_cents,
            rake_cents: input.rake_cents,
            seat_cap: input.seat_cap,
            starting_stack: input.starting_stack,
            early_bird_bonus_chips: input.early_bird_bonus_chips,
            level_two_bonus_chips: input.level_two_bonus_chips,
            voucher_value_cents: input.voucher_value_cents,
            rebuy_max: input.rebuy_max,
            addon_chips: input.addon_chips,
            addon_price_cents: input.addon_price_cents,
            late_registration_level: input.late_registration_level,
            bounty_type: input.bounty_type.map(String::from),
            bounty_amount_cents: input.bounty_amount_cents,
            leaderboard_config_id: input
                .leaderboard_config_id
                .as_ref()
                .map(|id| Uuid::parse_str(id.as_str()))
                .transpose()
                .gql_err("Invalid league ID")?,
        };

        let updated_row = tournaments::update(&state.db, tournament_id, data)
            .await
            .gql_err("Failed to update tournament")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found or already finished"))?;

        // Handle structure updates if provided
        if let Some(template_id) = input.template_id {
            let template_uuid =
                Uuid::parse_str(template_id.as_str()).gql_err("Invalid template ID")?;

            // Fetch template
            let template =
                infra::repos::blind_structure_templates::get_by_id(&state.db, template_uuid)
                    .await
                    .gql_err("Failed to fetch template")?
                    .ok_or_else(|| async_graphql::Error::new("Template not found"))?;

            // Deserialize levels from JSON
            let levels: Vec<crate::gql::domains::templates::types::BlindStructureLevel> =
                serde_json::from_value(template.levels)
                    .gql_err("Invalid template levels format")?;

            // Convert to TournamentStructureLevel
            let structure_levels: Vec<infra::repos::tournament_clock::TournamentStructureLevel> =
                levels
                    .into_iter()
                    .map(
                        |level| infra::repos::tournament_clock::TournamentStructureLevel {
                            level_number: level.level_number,
                            small_blind: level.small_blind,
                            big_blind: level.big_blind,
                            ante: level.ante,
                            duration_minutes: level.duration_minutes,
                            is_break: level.is_break,
                            break_duration_minutes: level.break_duration_minutes,
                        },
                    )
                    .collect();

            // Replace existing structure
            infra::repos::tournament_clock::replace_structures(
                &state.db,
                tournament_id,
                structure_levels,
            )
            .await
            .gql_err("Failed to replace structure")?;
        } else if let Some(custom_structure) = input.structure {
            // Convert input to structure level format
            let levels: Vec<infra::repos::tournament_clock::TournamentStructureLevel> =
                custom_structure
                    .into_iter()
                    .map(
                        |level_input| infra::repos::tournament_clock::TournamentStructureLevel {
                            level_number: level_input.level_number,
                            small_blind: level_input.small_blind,
                            big_blind: level_input.big_blind,
                            ante: level_input.ante,
                            duration_minutes: level_input.duration_minutes,
                            is_break: level_input.is_break,
                            break_duration_minutes: level_input.break_duration_minutes,
                        },
                    )
                    .collect();

            infra::repos::tournament_clock::replace_structures(&state.db, tournament_id, levels)
                .await
                .gql_err("Failed to replace structure")?;
        }

        Ok(Tournament::from(updated_row))
    }

    /// Update tournament live status
    async fn update_tournament_status(
        &self,
        ctx: &Context<'_>,
        input: UpdateTournamentStatusInput,
    ) -> Result<Tournament> {
        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Get tournament to check club_id
        let existing = tournaments::get_by_id(&state.db, tournament_id)
            .await
            .gql_err("Database operation failed")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        // Check permissions
        let _user = require_club_manager(ctx, existing.club_id).await?;

        // Update live status
        let live_status = input.live_status.into();
        let updated_row = tournaments::update_live_status(&state.db, tournament_id, live_status)
            .await
            .gql_err("Failed to update tournament status")?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        let manager_id = Uuid::parse_str(_user.id.as_str()).ok();

        // Log activity
        {
            let db = state.db.clone();
            let from_status = format!("{:?}", existing.live_status);
            let to_status = format!("{:?}", updated_row.live_status);
            tokio::spawn(async move {
                crate::gql::domains::activity_log::log_and_publish(
                    &db,
                    tournament_id,
                    "tournament",
                    "status_changed",
                    manager_id,
                    None,
                    serde_json::json!({"from_status": from_status, "to_status": to_status}),
                )
                .await;
            });
        }

        // Seat draw: when registration-open flips to late-registration, randomly
        // seat every checked-in player who still has no seat. Best-effort - a
        // seating failure must not fail the status change itself.
        if existing.live_status == TournamentLiveStatus::RegistrationOpen
            && updated_row.live_status == TournamentLiveStatus::LateRegistration
        {
            if let Some(manager_uuid) = manager_id {
                match crate::gql::domains::seating::service::auto_seat_checked_in(
                    &state.db,
                    tournament_id,
                    manager_uuid,
                )
                .await
                {
                    Ok(result) if !result.assignments.is_empty() => {
                        let count = result.assignments.len();
                        publish_seating_event(SeatingChangeEvent {
                            event_type: SeatingEventType::TablesBalanced,
                            tournament_id: tournament_id.into(),
                            club_id: existing.club_id.into(),
                            affected_assignment: None,
                            affected_player: None,
                            message: format!("{count} players auto-seated for late registration"),
                            timestamp: chrono::Utc::now(),
                        });

                        let db = state.db.clone();
                        tokio::spawn(async move {
                            crate::gql::domains::activity_log::log_and_publish(
                                &db,
                                tournament_id,
                                "seating",
                                "auto_seated",
                                manager_id,
                                None,
                                serde_json::json!({ "player_count": count }),
                            )
                            .await;
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(
                            tournament_id = %tournament_id,
                            error = %e,
                            "Auto-seat at late registration failed",
                        );
                    }
                }
            }
        }

        Ok(Tournament::from(updated_row))
    }
}
