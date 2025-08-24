use async_graphql::{Context, Object, Result};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use infra::{
    pagination::LimitOffset,
    repos::{
        ClubRepo, ClubTableRepo, LeaderboardPeriod, SeatAssignmentFilter, TableSeatAssignmentRepo,
        TournamentFilter, TournamentPayoutRepo, TournamentRegistrationRepo, TournamentRepo,
        TournamentResultRepo, UserFilter, UserRepo, UserStatistics,
    },
};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get tournament clock state
    async fn tournament_clock(
        &self,
        ctx: &Context<'_>,
        tournament_id: async_graphql::ID,
    ) -> Result<Option<crate::gql::types::TournamentClock>> {
        let query = crate::gql::tournament_clock::TournamentClockQuery;
        query.tournament_clock(ctx, tournament_id).await
    }

    /// Get tournament structure levels
    async fn tournament_structure(
        &self,
        ctx: &Context<'_>,
        tournament_id: async_graphql::ID,
    ) -> Result<Vec<crate::gql::types::TournamentStructure>> {
        let query = crate::gql::tournament_clock::TournamentClockQuery;
        query.tournament_structure(ctx, tournament_id).await
    }
    /// Current server time (UTC), example of returning chrono types.
    async fn server_time(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn clubs(&self, ctx: &Context<'_>) -> Result<Vec<crate::gql::types::Club>> {
        let state = ctx.data::<AppState>()?;
        let repo = ClubRepo::new(state.db.clone());
        let rows = repo.list_all().await?;
        Ok(rows
            .into_iter()
            .map(|r| crate::gql::types::Club {
                id: r.id.into(),
                name: r.name,
                city: r.city,
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    async fn tournaments(
        &self,
        ctx: &async_graphql::Context<'_>,
        club_id: Option<uuid::Uuid>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
        status: Option<crate::gql::types::TournamentStatus>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> async_graphql::Result<Vec<crate::gql::types::Tournament>> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentRepo::new(state.db.clone());
        let filter = TournamentFilter {
            club_id,
            from,
            to,
            status: status.map(|s| s.into()),
        };
        let page = Some(LimitOffset {
            limit: limit.unwrap_or(50).clamp(1, 200),
            offset: offset.unwrap_or(0).max(0),
        });
        let rows = repo.list(filter, page).await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let status = r.calculate_status().into();
                crate::gql::types::Tournament {
                    id: r.id.into(),
                    title: r.name.clone(),
                    description: r.description.clone(),
                    club_id: r.club_id.into(),
                    start_time: r.start_time,
                    end_time: r.end_time,
                    buy_in_cents: r.buy_in_cents,
                    seat_cap: r.seat_cap,
                    status,
                    live_status: r.live_status.into(),
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }
            })
            .collect())
    }

    async fn tournament_state(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
    ) -> Result<Option<crate::gql::types::TournamentState>> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentRepo::new(state.db.clone());

        match repo.get_state(tournament_id).await? {
            Some(state_row) => Ok(Some(crate::gql::types::TournamentState {
                id: state_row.id.into(),
                tournament_id: state_row.tournament_id.into(),
                current_level: state_row.current_level,
                players_remaining: state_row.players_remaining,
                break_until: state_row.break_until,
                current_small_blind: state_row.current_small_blind,
                current_big_blind: state_row.current_big_blind,
                current_ante: state_row.current_ante,
                level_started_at: state_row.level_started_at,
                level_duration_minutes: state_row.level_duration_minutes,
                created_at: state_row.created_at,
                updated_at: state_row.updated_at,
            })),
            None => Ok(None),
        }
    }

    async fn users(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
        is_active: Option<bool>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<crate::gql::types::User>> {
        let state = ctx.data::<AppState>()?;
        let repo = UserRepo::new(state.db.clone());
        let filter = UserFilter { search, is_active };
        let page = Some(LimitOffset {
            limit: limit.unwrap_or(50).clamp(1, 200),
            offset: offset.unwrap_or(0).max(0),
        });
        let rows = repo.list(filter, page).await?;
        Ok(rows
            .into_iter()
            .map(|r| crate::gql::types::User {
                id: r.id.into(),
                email: r.email,
                username: r.username,
                first_name: r.first_name,
                last_name: r.last_name,
                phone: r.phone,
                is_active: r.is_active,
                role: crate::gql::types::Role::from(r.role),
            })
            .collect())
    }

    async fn tournament_players(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
    ) -> Result<Vec<crate::gql::types::TournamentPlayer>> {
        let state = ctx.data::<AppState>()?;
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());
        let user_repo = UserRepo::new(state.db.clone());

        let registrations = registration_repo.get_by_tournament(tournament_id).await?;

        let mut players = Vec::new();
        for registration in registrations {
            if let Some(user_row) = user_repo.get_by_id(registration.user_id).await? {
                let tournament_registration = crate::gql::types::TournamentRegistration {
                    id: registration.id.into(),
                    tournament_id: registration.tournament_id.into(),
                    user_id: registration.user_id.into(),
                    registration_time: registration.registration_time,
                    status: registration.status,
                    notes: registration.notes,
                };

                let user = crate::gql::types::User {
                    id: user_row.id.into(),
                    email: user_row.email,
                    username: user_row.username,
                    first_name: user_row.first_name,
                    last_name: user_row.last_name,
                    phone: user_row.phone,
                    is_active: user_row.is_active,
                    role: crate::gql::types::Role::from(user_row.role),
                };

                players.push(crate::gql::types::TournamentPlayer {
                    registration: tournament_registration,
                    user,
                });
            }
        }

        Ok(players)
    }

    /// Get the current authenticated user's information
    async fn me(&self, ctx: &Context<'_>) -> Result<crate::gql::types::User> {
        use crate::auth::Claims;

        // Get authenticated user from JWT token
        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let user_repo = UserRepo::new(state.db.clone());

        let user = user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("User not found"))?;

        Ok(crate::gql::types::User {
            id: user.id.into(),
            email: user.email,
            username: user.username,
            first_name: user.first_name,
            last_name: user.last_name,
            phone: user.phone,
            is_active: user.is_active,
            role: crate::gql::types::Role::from(user.role),
        })
    }

    async fn my_tournament_registrations(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<crate::gql::types::TournamentRegistration>> {
        use crate::auth::Claims;

        // Get authenticated user from JWT token
        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());

        let registrations = registration_repo
            .get_user_current_registrations(user_id)
            .await?;

        Ok(registrations
            .into_iter()
            .map(|r| crate::gql::types::TournamentRegistration {
                id: r.id.into(),
                tournament_id: r.tournament_id.into(),
                user_id: r.user_id.into(),
                registration_time: r.registration_time,
                status: r.status,
                notes: r.notes,
            })
            .collect())
    }

    async fn my_recent_tournament_results(
        &self,
        ctx: &Context<'_>,
        limit: Option<i64>,
    ) -> Result<Vec<crate::gql::types::UserTournamentResult>> {
        use crate::auth::Claims;

        // Get authenticated user from JWT token
        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let result_repo = TournamentResultRepo::new(state.db.clone());
        let tournament_repo = TournamentRepo::new(state.db.clone());

        let limit = limit.unwrap_or(10).clamp(1, 50);
        let results = result_repo.get_user_recent_results(user_id, limit).await?;

        let mut user_results = Vec::new();
        for result_row in results {
            if let Some(tournament_row) = tournament_repo.get(result_row.tournament_id).await? {
                let tournament_result = crate::gql::types::TournamentResult {
                    id: result_row.id.into(),
                    tournament_id: result_row.tournament_id.into(),
                    user_id: result_row.user_id.into(),
                    final_position: result_row.final_position,
                    prize_cents: result_row.prize_cents,
                    points: result_row.points,
                    notes: result_row.notes,
                    created_at: result_row.created_at,
                };

                let status = tournament_row.calculate_status().into();
                let tournament = crate::gql::types::Tournament {
                    id: tournament_row.id.into(),
                    title: tournament_row.name.clone(),
                    description: tournament_row.description.clone(),
                    club_id: tournament_row.club_id.into(),
                    start_time: tournament_row.start_time,
                    end_time: tournament_row.end_time,
                    buy_in_cents: tournament_row.buy_in_cents,
                    seat_cap: tournament_row.seat_cap,
                    status,
                    live_status: tournament_row.live_status.into(),
                    created_at: tournament_row.created_at,
                    updated_at: tournament_row.updated_at,
                };

                user_results.push(crate::gql::types::UserTournamentResult {
                    result: tournament_result,
                    tournament,
                });
            }
        }

        Ok(user_results)
    }

    async fn my_tournament_statistics(
        &self,
        ctx: &Context<'_>,
    ) -> Result<crate::gql::types::PlayerStatsResponse> {
        use crate::auth::Claims;

        // Get authenticated user from JWT token
        let claims = ctx
            .data::<Claims>()
            .map_err(|_| async_graphql::Error::new("Authentication required"))?;

        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| async_graphql::Error::new(format!("Invalid user ID: {}", e)))?;

        let state = ctx.data::<AppState>()?;
        let result_repo = TournamentResultRepo::new(state.db.clone());

        // Get statistics for different time periods
        let stats_7_days = result_repo.get_user_statistics(user_id, 7).await?;
        let stats_30_days = result_repo.get_user_statistics(user_id, 30).await?;
        let stats_year = result_repo.get_user_statistics(user_id, 365).await?;

        // Convert to GraphQL types
        let convert_stats = |stats: UserStatistics| crate::gql::types::PlayerStatistics {
            total_itm: stats.total_itm,
            total_tournaments: stats.total_tournaments,
            total_winnings: stats.total_winnings,
            total_buy_ins: stats.total_buy_ins,
            itm_percentage: stats.itm_percentage,
            roi_percentage: stats.roi_percentage,
        };

        Ok(crate::gql::types::PlayerStatsResponse {
            last_7_days: convert_stats(stats_7_days),
            last_30_days: convert_stats(stats_30_days),
            last_year: convert_stats(stats_year),
        })
    }

    /// Get the current seating chart for a tournament
    async fn tournament_seating_chart(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
    ) -> Result<crate::gql::types::TournamentSeatingChart> {
        let state = ctx.data::<AppState>()?;
        let tournament_repo = TournamentRepo::new(state.db.clone());
        let club_table_repo = ClubTableRepo::new(state.db.clone());
        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());

        // Get tournament
        let tournament_row = tournament_repo
            .get(tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        let tournament = crate::gql::types::Tournament {
            id: tournament_row.id.into(),
            title: tournament_row.name.clone(),
            description: tournament_row.description.clone(),
            club_id: tournament_row.club_id.into(),
            start_time: tournament_row.start_time,
            end_time: tournament_row.end_time,
            buy_in_cents: tournament_row.buy_in_cents,
            seat_cap: tournament_row.seat_cap,
            status: tournament_row.calculate_status().into(),
            live_status: tournament_row.live_status.into(),
            created_at: tournament_row.created_at,
            updated_at: tournament_row.updated_at,
        };

        // Get all active tables for the tournament
        let table_rows = club_table_repo
            .get_assigned_to_tournament(tournament_id)
            .await?;

        // For each table, get current seat assignments with player info
        let mut tables = Vec::new();
        for table_row in table_rows {
            let table = crate::gql::types::TournamentTable {
                id: table_row.id.into(),
                tournament_id: tournament_id.into(), // Use the parameter since club table doesn't have tournament_id
                table_number: table_row.table_number,
                max_seats: table_row.max_seats,
                is_active: table_row.is_active,
                table_name: table_row.table_name,
                created_at: table_row.created_at,
            };

            let assignments_with_players = assignment_repo
                .get_current_with_players_for_table(table_row.id)
                .await?;
            let seats: Vec<crate::gql::types::SeatWithPlayer> = assignments_with_players
                .into_iter()
                .map(|ap| {
                    crate::gql::types::SeatWithPlayer {
                        assignment: crate::gql::types::SeatAssignment {
                            id: ap.assignment.id.into(),
                            tournament_id: ap.assignment.tournament_id.into(),
                            club_table_id: ap.assignment.club_table_id.into(),
                            user_id: ap.assignment.user_id.into(),
                            seat_number: ap.assignment.seat_number,
                            stack_size: ap.assignment.stack_size,
                            is_current: ap.assignment.is_current,
                            assigned_at: ap.assignment.assigned_at,
                            unassigned_at: None, // Field not yet implemented in database
                            assigned_by: None,   // Field not yet implemented in database
                            notes: None,         // Field not yet implemented in database
                        },
                        player: crate::gql::types::User {
                            id: ap.player.id.into(),
                            email: ap.player.email,
                            username: ap.player.username,
                            first_name: ap.player.first_name,
                            last_name: ap.player.last_name,
                            phone: ap.player.phone,
                            is_active: ap.player.is_active,
                            role: crate::gql::types::Role::from(ap.player.role),
                        },
                    }
                })
                .collect();

            tables.push(crate::gql::types::TableWithSeats { table, seats });
        }

        // Get unassigned players
        let unassigned_player_rows = assignment_repo
            .get_unassigned_players(tournament_id)
            .await?;
        let unassigned_players: Vec<crate::gql::types::User> = unassigned_player_rows
            .into_iter()
            .map(|p| crate::gql::types::User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: crate::gql::types::Role::from(p.role),
            })
            .collect();

        Ok(crate::gql::types::TournamentSeatingChart {
            tournament,
            tables,
            unassigned_players,
        })
    }

    /// Get all tables assigned to a tournament (from club tables)
    async fn tournament_tables(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
    ) -> Result<Vec<crate::gql::types::TournamentTable>> {
        let state = ctx.data::<AppState>()?;
        let club_table_repo = ClubTableRepo::new(state.db.clone());

        let table_rows = club_table_repo
            .get_assigned_to_tournament(tournament_id)
            .await?;

        Ok(table_rows
            .into_iter()
            .map(|table_row| crate::gql::types::TournamentTable {
                id: table_row.id.into(),
                tournament_id: tournament_id.into(), // Use the tournament_id parameter
                table_number: table_row.table_number,
                max_seats: table_row.max_seats,
                is_active: table_row.is_active,
                table_name: table_row.table_name,
                created_at: table_row.created_at,
            })
            .collect())
    }

    /// Get all tables for a club
    async fn club_tables(
        &self,
        ctx: &Context<'_>,
        club_id: uuid::Uuid,
    ) -> Result<Vec<crate::gql::types::ClubTable>> {
        let state = ctx.data::<AppState>()?;
        let club_table_repo = ClubTableRepo::new(state.db.clone());

        let table_rows = club_table_repo.get_by_club(club_id).await?;

        Ok(table_rows
            .into_iter()
            .map(|table_row| crate::gql::types::ClubTable {
                id: table_row.id.into(),
                club_id: table_row.club_id.into(),
                table_number: table_row.table_number,
                max_seats: table_row.max_seats,
                table_name: table_row.table_name,
                location: table_row.location,
                is_active: table_row.is_active,
                created_at: table_row.created_at,
                updated_at: table_row.updated_at,
            })
            .collect())
    }

    /// Get current seat assignments for a specific table
    async fn table_seat_assignments(
        &self,
        ctx: &Context<'_>,
        club_table_id: uuid::Uuid,
    ) -> Result<Vec<crate::gql::types::SeatWithPlayer>> {
        let state = ctx.data::<AppState>()?;
        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());

        let assignments_with_players = assignment_repo
            .get_current_with_players_for_table(club_table_id)
            .await?;

        Ok(assignments_with_players
            .into_iter()
            .map(|ap| {
                crate::gql::types::SeatWithPlayer {
                    assignment: crate::gql::types::SeatAssignment {
                        id: ap.assignment.id.into(),
                        tournament_id: ap.assignment.tournament_id.into(),
                        club_table_id: ap.assignment.club_table_id.into(),
                        user_id: ap.assignment.user_id.into(),
                        seat_number: ap.assignment.seat_number,
                        stack_size: ap.assignment.stack_size,
                        is_current: ap.assignment.is_current,
                        assigned_at: ap.assignment.assigned_at,
                        unassigned_at: None, // Field not yet implemented in database
                        assigned_by: None,   // Field not yet implemented in database
                        notes: None,         // Field not yet implemented in database
                    },
                    player: crate::gql::types::User {
                        id: ap.player.id.into(),
                        email: ap.player.email,
                        username: ap.player.username,
                        first_name: ap.player.first_name,
                        last_name: ap.player.last_name,
                        phone: ap.player.phone,
                        is_active: ap.player.is_active,
                        role: crate::gql::types::Role::from(ap.player.role),
                    },
                }
            })
            .collect())
    }

    /// Get seating history for a tournament (useful for tracking moves)
    /// Get complete tournament data - static info, live state, players, and seating
    async fn tournament_complete(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
    ) -> Result<crate::gql::types::TournamentComplete> {
        let state = ctx.data::<AppState>()?;
        let tournament_repo = TournamentRepo::new(state.db.clone());
        let registration_repo = TournamentRegistrationRepo::new(state.db.clone());

        // Get tournament with all static data
        let tournament_row = tournament_repo
            .get(tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        let tournament = crate::gql::types::Tournament {
            id: tournament_row.id.into(),
            title: tournament_row.name.clone(),
            description: tournament_row.description.clone(),
            club_id: tournament_row.club_id.into(),
            start_time: tournament_row.start_time,
            end_time: tournament_row.end_time,
            buy_in_cents: tournament_row.buy_in_cents,
            seat_cap: tournament_row.seat_cap,
            status: tournament_row.calculate_status().into(),
            live_status: tournament_row.live_status.into(),
            created_at: tournament_row.created_at,
            updated_at: tournament_row.updated_at,
        };

        // Get live state
        let live_state = tournament_repo
            .get_state(tournament_id)
            .await?
            .map(|state_row| crate::gql::types::TournamentState {
                id: state_row.id.into(),
                tournament_id: state_row.tournament_id.into(),
                current_level: state_row.current_level,
                players_remaining: state_row.players_remaining,
                break_until: state_row.break_until,
                current_small_blind: state_row.current_small_blind,
                current_big_blind: state_row.current_big_blind,
                current_ante: state_row.current_ante,
                level_started_at: state_row.level_started_at,
                level_duration_minutes: state_row.level_duration_minutes,
                created_at: state_row.created_at,
                updated_at: state_row.updated_at,
            });

        // Get registrations count
        let registrations = registration_repo.get_by_tournament(tournament_id).await?;
        let total_registered = registrations.len() as i32;

        Ok(crate::gql::types::TournamentComplete {
            tournament,
            live_state,
            total_registered,
        })
    }

    async fn tournament_seating_history(
        &self,
        ctx: &Context<'_>,
        tournament_id: uuid::Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<crate::gql::types::SeatAssignment>> {
        let state = ctx.data::<AppState>()?;
        let assignment_repo = TableSeatAssignmentRepo::new(state.db.clone());

        let filter = SeatAssignmentFilter {
            tournament_id: Some(tournament_id),
            club_table_id: None,
            user_id: None,
            is_current: None, // Show both current and historical
            from_date: None,
            to_date: None,
        };

        let assignment_rows = assignment_repo.get_history(filter, limit).await?;

        Ok(assignment_rows
            .into_iter()
            .map(|assignment| {
                crate::gql::types::SeatAssignment {
                    id: assignment.id.into(),
                    tournament_id: assignment.tournament_id.into(),
                    club_table_id: assignment.club_table_id.into(),
                    user_id: assignment.user_id.into(),
                    seat_number: assignment.seat_number,
                    stack_size: assignment.stack_size,
                    is_current: assignment.is_current,
                    assigned_at: assignment.assigned_at,
                    unassigned_at: None, // Field not yet implemented in database
                    assigned_by: None,   // Field not yet implemented in database
                    notes: None,         // Field not yet implemented in database
                }
            })
            .collect())
    }

    /// Get tournament payout structure
    async fn tournament_payout(
        &self,
        ctx: &Context<'_>,
        tournament_id: async_graphql::ID,
    ) -> Result<Option<crate::gql::types::TournamentPayout>> {
        let state = ctx.data::<AppState>()?;
        let repo = TournamentPayoutRepo::new(state.db.clone());

        let tournament_id = uuid::Uuid::parse_str(tournament_id.as_str())
            .map_err(|e| async_graphql::Error::new(format!("Invalid tournament ID: {}", e)))?;

        if let Some(payout_row) = repo.get_by_tournament(tournament_id).await? {
            // Parse the JSONB payout_positions into structured data
            let positions_array = payout_row
                .payout_positions
                .as_array()
                .ok_or_else(|| async_graphql::Error::new("Invalid payout positions format"))?;

            let mut positions = Vec::new();
            for pos in positions_array {
                let position = pos
                    .get("position")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| async_graphql::Error::new("Invalid position value"))?
                    as i32;

                let percentage = pos
                    .get("percentage")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| async_graphql::Error::new("Invalid percentage value"))?;

                let amount_cents = pos
                    .get("amount_cents")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| async_graphql::Error::new("Invalid amount_cents value"))?
                    as i32;

                positions.push(crate::gql::types::PayoutPosition {
                    position,
                    percentage,
                    amount_cents,
                });
            }

            // Sort positions by position number
            positions.sort_by_key(|p| p.position);

            Ok(Some(crate::gql::types::TournamentPayout {
                id: payout_row.id.into(),
                tournament_id: payout_row.tournament_id.into(),
                template_id: payout_row.template_id.map(|id| id.into()),
                player_count: payout_row.player_count,
                total_prize_pool: payout_row.total_prize_pool,
                positions,
                created_at: payout_row.created_at,
                updated_at: payout_row.updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get player leaderboard with comprehensive statistics and points
    async fn leaderboard(
        &self,
        ctx: &Context<'_>,
        period: Option<crate::gql::types::LeaderboardPeriod>,
        limit: Option<i32>,
        club_id: Option<uuid::Uuid>,
    ) -> Result<crate::gql::types::LeaderboardResponse> {
        let state = ctx.data::<AppState>()?;
        let result_repo = TournamentResultRepo::new(state.db.clone());

        let period = period.unwrap_or(crate::gql::types::LeaderboardPeriod::AllTime);
        let infra_period: LeaderboardPeriod = period.into();

        let leaderboard_entries = result_repo
            .get_leaderboard(infra_period, limit, club_id)
            .await?;

        // Convert to GraphQL types and add rank
        let entries: Vec<crate::gql::types::LeaderboardEntry> = leaderboard_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| crate::gql::types::LeaderboardEntry {
                user: crate::gql::types::User {
                    id: entry.user_id.into(),
                    email: entry.email,
                    username: entry.username,
                    first_name: entry.first_name,
                    last_name: entry.last_name,
                    phone: entry.phone,
                    is_active: entry.is_active,
                    role: crate::gql::types::Role::from(entry.role),
                },
                rank: (index + 1) as i32, // 1-based ranking
                total_tournaments: entry.total_tournaments,
                total_buy_ins: entry.total_buy_ins,
                total_winnings: entry.total_winnings,
                net_profit: entry.net_profit,
                total_itm: entry.total_itm,
                itm_percentage: entry.itm_percentage,
                roi_percentage: entry.roi_percentage,
                average_finish: entry.average_finish,
                first_places: entry.first_places,
                final_tables: entry.final_tables,
                points: entry.points,
            })
            .collect();

        let total_players = entries.len() as i32;

        Ok(crate::gql::types::LeaderboardResponse {
            entries,
            total_players,
            period,
        })
    }
}
