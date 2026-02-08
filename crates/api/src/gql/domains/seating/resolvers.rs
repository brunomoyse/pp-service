use async_graphql::{Context, Object, Result, ID};
use uuid::Uuid;

use crate::gql::error::ResultExt;
use crate::gql::subscriptions::publish_seating_event;
use crate::gql::types::{
    AssignPlayerToSeatInput, AssignTableToTournamentInput, BalanceTablesInput, MovePlayerInput,
    Role, SeatAssignment, SeatWithPlayer, SeatingChangeEvent, SeatingEventType, TableWithSeats,
    Tournament, TournamentSeatingChart, TournamentTable, UnassignTableFromTournamentInput,
    UpdateStackSizeInput, User,
};
use crate::state::AppState;
use infra::repos::{
    club_tables, table_seat_assignments, table_seat_assignments::CreateSeatAssignment,
    table_seat_assignments::SeatAssignmentFilter, table_seat_assignments::UpdateSeatAssignment,
    tournaments, users,
};

// Helper function to check if tables need rebalancing
fn needs_rebalancing(table_counts: &std::collections::HashMap<Uuid, usize>) -> bool {
    if table_counts.is_empty() {
        return false;
    }

    let min_count = *table_counts.values().min().unwrap_or(&0);
    let max_count = *table_counts.values().max().unwrap_or(&0);

    // Rebalance if difference is more than 2 players between tables
    // OR if any table has less than 4 players (unless it's the only table)
    (max_count - min_count > 2) || (min_count < 4 && table_counts.len() > 1)
}

async fn get_club_id_for_tournament(db: &infra::db::Db, tournament_id: Uuid) -> Result<Uuid> {
    let tournament = tournaments::get_by_id(db, tournament_id)
        .await?
        .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;
    Ok(tournament.club_id)
}

#[derive(Default)]
pub struct SeatingQuery;

#[Object]
impl SeatingQuery {
    /// Get the current seating chart for a tournament
    async fn tournament_seating_chart(
        &self,
        ctx: &Context<'_>,
        tournament_id: Uuid,
    ) -> Result<TournamentSeatingChart> {
        let state = ctx.data::<AppState>()?;

        // Get tournament
        let tournament_row = tournaments::get_by_id(&state.db, tournament_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Tournament not found"))?;

        let tournament = Tournament {
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
            early_bird_bonus_chips: tournament_row.early_bird_bonus_chips,
            created_at: tournament_row.created_at,
            updated_at: tournament_row.updated_at,
        };

        // Get all active tables for the tournament
        let table_rows = club_tables::list_assigned_to_tournament(&state.db, tournament_id).await?;

        // For each table, get current seat assignments with player info
        let mut tables = Vec::new();
        for table_row in table_rows {
            let table = TournamentTable {
                id: table_row.id.into(),
                tournament_id: tournament_id.into(),
                table_number: table_row.table_number,
                max_seats: table_row.max_seats,
                is_active: table_row.is_active,
                created_at: table_row.created_at,
            };

            let assignments_with_players =
                table_seat_assignments::list_current_with_players_for_table(
                    &state.db,
                    table_row.id,
                )
                .await?;
            let seats: Vec<SeatWithPlayer> = assignments_with_players
                .into_iter()
                .map(|ap| SeatWithPlayer {
                    assignment: SeatAssignment {
                        id: ap.assignment.id.into(),
                        tournament_id: ap.assignment.tournament_id.into(),
                        club_table_id: ap.assignment.club_table_id.into(),
                        user_id: ap.assignment.user_id.into(),
                        seat_number: ap.assignment.seat_number,
                        stack_size: ap.assignment.stack_size,
                        is_current: ap.assignment.is_current,
                        assigned_at: ap.assignment.assigned_at,
                        unassigned_at: ap.assignment.unassigned_at,
                        assigned_by: ap.assignment.assigned_by.map(|id| id.into()),
                        notes: ap.assignment.notes,
                    },
                    player: User {
                        id: ap.player.id.into(),
                        email: ap.player.email,
                        username: ap.player.username,
                        first_name: ap.player.first_name,
                        last_name: ap.player.last_name,
                        phone: ap.player.phone,
                        is_active: ap.player.is_active,
                        role: Role::from(ap.player.role),
                    },
                })
                .collect();

            tables.push(TableWithSeats { table, seats });
        }

        // Get unassigned players
        let unassigned_player_rows =
            table_seat_assignments::list_unassigned_players(&state.db, tournament_id).await?;
        let unassigned_players: Vec<User> = unassigned_player_rows
            .into_iter()
            .map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: Role::from(p.role),
            })
            .collect();

        Ok(TournamentSeatingChart {
            tournament,
            tables,
            unassigned_players,
        })
    }

    /// Get all tables assigned to a tournament (from club tables)
    async fn tournament_tables(
        &self,
        ctx: &Context<'_>,
        tournament_id: Uuid,
    ) -> Result<Vec<TournamentTable>> {
        let state = ctx.data::<AppState>()?;

        let table_rows = club_tables::list_assigned_to_tournament(&state.db, tournament_id).await?;

        Ok(table_rows
            .into_iter()
            .map(|table_row| TournamentTable {
                id: table_row.id.into(),
                tournament_id: tournament_id.into(),
                table_number: table_row.table_number,
                max_seats: table_row.max_seats,
                is_active: table_row.is_active,
                created_at: table_row.created_at,
            })
            .collect())
    }

    /// Get current seat assignments for a specific table
    async fn table_seat_assignments(
        &self,
        ctx: &Context<'_>,
        club_table_id: Uuid,
    ) -> Result<Vec<SeatWithPlayer>> {
        let state = ctx.data::<AppState>()?;

        let assignments_with_players =
            table_seat_assignments::list_current_with_players_for_table(&state.db, club_table_id)
                .await?;

        Ok(assignments_with_players
            .into_iter()
            .map(|ap| SeatWithPlayer {
                assignment: SeatAssignment {
                    id: ap.assignment.id.into(),
                    tournament_id: ap.assignment.tournament_id.into(),
                    club_table_id: ap.assignment.club_table_id.into(),
                    user_id: ap.assignment.user_id.into(),
                    seat_number: ap.assignment.seat_number,
                    stack_size: ap.assignment.stack_size,
                    is_current: ap.assignment.is_current,
                    assigned_at: ap.assignment.assigned_at,
                    unassigned_at: ap.assignment.unassigned_at,
                    assigned_by: ap.assignment.assigned_by.map(|id| id.into()),
                    notes: ap.assignment.notes,
                },
                player: User {
                    id: ap.player.id.into(),
                    email: ap.player.email,
                    username: ap.player.username,
                    first_name: ap.player.first_name,
                    last_name: ap.player.last_name,
                    phone: ap.player.phone,
                    is_active: ap.player.is_active,
                    role: Role::from(ap.player.role),
                },
            })
            .collect())
    }

    async fn tournament_seating_history(
        &self,
        ctx: &Context<'_>,
        tournament_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<SeatAssignment>> {
        let state = ctx.data::<AppState>()?;

        let filter = SeatAssignmentFilter {
            tournament_id: Some(tournament_id),
            club_table_id: None,
            user_id: None,
            is_current: None, // Show both current and historical
            from_date: None,
            to_date: None,
        };

        let assignment_rows =
            table_seat_assignments::list_history(&state.db, filter, limit).await?;

        Ok(assignment_rows
            .into_iter()
            .map(|assignment| SeatAssignment {
                id: assignment.id.into(),
                tournament_id: assignment.tournament_id.into(),
                club_table_id: assignment.club_table_id.into(),
                user_id: assignment.user_id.into(),
                seat_number: assignment.seat_number,
                stack_size: assignment.stack_size,
                is_current: assignment.is_current,
                assigned_at: assignment.assigned_at,
                unassigned_at: assignment.unassigned_at,
                assigned_by: assignment.assigned_by.map(|id| id.into()),
                notes: assignment.notes,
            })
            .collect())
    }
}

#[derive(Default)]
pub struct SeatingMutation;

#[Object]
impl SeatingMutation {
    /// Assign a club table to a tournament (managers only)
    async fn assign_table_to_tournament(
        &self,
        ctx: &Context<'_>,
        input: AssignTableToTournamentInput,
    ) -> Result<TournamentTable> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let club_table_id =
            Uuid::parse_str(input.club_table_id.as_str()).gql_err("Invalid club table ID")?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let _manager = require_club_manager(ctx, club_id).await?;

        // Verify the club table belongs to the same club as the tournament
        let club_table = club_tables::get_by_id(&state.db, club_table_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Club table not found"))?;

        if club_table.club_id != club_id {
            return Err(async_graphql::Error::new(
                "Club table does not belong to the tournament's club",
            ));
        }

        // Assign the table to the tournament
        let _assignment =
            club_tables::assign_to_tournament(&state.db, tournament_id, club_table_id).await?;

        // Publish seating change event
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::TableCreated,
            tournament_id: tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: None,
            affected_player: None,
            message: format!("Table {} assigned to tournament", club_table.table_number),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(TournamentTable {
            id: club_table.id.into(),
            tournament_id: tournament_id.into(),
            table_number: club_table.table_number,
            max_seats: club_table.max_seats,
            is_active: club_table.is_active,
            created_at: club_table.created_at,
        })
    }

    /// Unassign (break) a table from a tournament (managers only)
    /// Only empty tables (with no seated players) can be unassigned
    async fn unassign_table_from_tournament(
        &self,
        ctx: &Context<'_>,
        input: UnassignTableFromTournamentInput,
    ) -> Result<bool> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let club_table_id =
            Uuid::parse_str(input.club_table_id.as_str()).gql_err("Invalid club table ID")?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let _manager = require_club_manager(ctx, club_id).await?;

        // Verify the club table exists and belongs to the same club
        let club_table = club_tables::get_by_id(&state.db, club_table_id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Club table not found"))?;

        if club_table.club_id != club_id {
            return Err(async_graphql::Error::new(
                "Club table does not belong to the tournament's club",
            ));
        }

        // Check if there are any active seat assignments on this table for this tournament
        let current_assignments = table_seat_assignments::list_current_for_tournament_table(
            &state.db,
            tournament_id,
            club_table_id,
        )
        .await?;

        if !current_assignments.is_empty() {
            return Err(async_graphql::Error::new(
                "Cannot unassign table: there are still players seated at this table. Move or eliminate all players first.",
            ));
        }

        // Unassign the table from the tournament
        let success =
            club_tables::unassign_from_tournament(&state.db, tournament_id, club_table_id).await?;

        if success {
            // Publish seating change event
            let event = SeatingChangeEvent {
                event_type: SeatingEventType::TableRemoved,
                tournament_id: tournament_id.into(),
                club_id: club_id.into(),
                affected_assignment: None,
                affected_player: None,
                message: format!("Table {} removed from tournament", club_table.table_number),
                timestamp: chrono::Utc::now(),
            };
            publish_seating_event(event);
        }

        Ok(success)
    }

    /// Assign a player to a specific seat (managers only)
    async fn assign_player_to_seat(
        &self,
        ctx: &Context<'_>,
        input: AssignPlayerToSeatInput,
    ) -> Result<SeatAssignment> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let manager = require_club_manager(ctx, club_id).await?;

        let club_table_id =
            Uuid::parse_str(input.club_table_id.as_str()).gql_err("Invalid table ID")?;
        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Check if seat is available
        let is_available =
            table_seat_assignments::is_seat_available(&state.db, club_table_id, input.seat_number)
                .await?;
        if !is_available {
            return Err(async_graphql::Error::new("Seat is already occupied"));
        }

        let create_data = CreateSeatAssignment {
            tournament_id,
            club_table_id,
            user_id,
            seat_number: input.seat_number,
            stack_size: input.stack_size,
            assigned_by: Some(manager_id),
            notes: input.notes,
        };

        let assignment_row = table_seat_assignments::create(&state.db, create_data).await?;

        // Get player info for the event
        let player = users::get_by_id(&state.db, user_id).await?;

        // Publish seating change event
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::PlayerAssigned,
            tournament_id: assignment_row.tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: Some(SeatAssignment {
                id: assignment_row.id.into(),
                tournament_id: assignment_row.tournament_id.into(),
                club_table_id: assignment_row.club_table_id.into(),
                user_id: assignment_row.user_id.into(),
                seat_number: assignment_row.seat_number,
                stack_size: assignment_row.stack_size,
                is_current: assignment_row.is_current,
                assigned_at: assignment_row.assigned_at,
                unassigned_at: assignment_row.unassigned_at,
                assigned_by: assignment_row.assigned_by.map(|id| id.into()),
                notes: assignment_row.notes.clone(),
            }),
            affected_player: player.map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: Role::from(p.role),
            }),
            message: format!("Player assigned to seat {}", assignment_row.seat_number),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(SeatAssignment {
            id: assignment_row.id.into(),
            tournament_id: assignment_row.tournament_id.into(),
            club_table_id: assignment_row.club_table_id.into(),
            user_id: assignment_row.user_id.into(),
            seat_number: assignment_row.seat_number,
            stack_size: assignment_row.stack_size,
            is_current: assignment_row.is_current,
            assigned_at: assignment_row.assigned_at,
            unassigned_at: assignment_row.unassigned_at,
            assigned_by: assignment_row.assigned_by.map(|id| id.into()),
            notes: assignment_row.notes,
        })
    }

    /// Move a player to a different table/seat (managers only)
    async fn move_player(
        &self,
        ctx: &Context<'_>,
        input: MovePlayerInput,
    ) -> Result<SeatAssignment> {
        use crate::auth::permissions::require_club_manager;

        let state = ctx.data::<AppState>()?;
        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;

        // Get club ID for the tournament to verify permissions
        let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;

        // Require manager role for this specific club
        let manager = require_club_manager(ctx, club_id).await?;

        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;
        let new_club_table_id =
            Uuid::parse_str(input.new_club_table_id.as_str()).gql_err("Invalid table ID")?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Check if new seat is available
        let is_available = table_seat_assignments::is_seat_available(
            &state.db,
            new_club_table_id,
            input.new_seat_number,
        )
        .await?;
        if !is_available {
            return Err(async_graphql::Error::new("Target seat is already occupied"));
        }

        let assignment_row = table_seat_assignments::move_player(
            &state.db,
            tournament_id,
            user_id,
            new_club_table_id,
            input.new_seat_number,
            Some(manager_id),
            input.notes,
        )
        .await?;

        // Get player info for the event
        let player = users::get_by_id(&state.db, user_id).await?;

        // Publish seating change event
        let club_id = get_club_id_for_tournament(&state.db, assignment_row.tournament_id).await?;
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::PlayerMoved,
            tournament_id: assignment_row.tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: Some(SeatAssignment {
                id: assignment_row.id.into(),
                tournament_id: assignment_row.tournament_id.into(),
                club_table_id: assignment_row.club_table_id.into(),
                user_id: assignment_row.user_id.into(),
                seat_number: assignment_row.seat_number,
                stack_size: assignment_row.stack_size,
                is_current: assignment_row.is_current,
                assigned_at: assignment_row.assigned_at,
                unassigned_at: assignment_row.unassigned_at,
                assigned_by: assignment_row.assigned_by.map(|id| id.into()),
                notes: assignment_row.notes.clone(),
            }),
            affected_player: player.map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: Role::from(p.role),
            }),
            message: format!("Player moved to seat {}", assignment_row.seat_number),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(SeatAssignment {
            id: assignment_row.id.into(),
            tournament_id: assignment_row.tournament_id.into(),
            club_table_id: assignment_row.club_table_id.into(),
            user_id: assignment_row.user_id.into(),
            seat_number: assignment_row.seat_number,
            stack_size: assignment_row.stack_size,
            is_current: assignment_row.is_current,
            assigned_at: assignment_row.assigned_at,
            unassigned_at: assignment_row.unassigned_at,
            assigned_by: assignment_row.assigned_by.map(|id| id.into()),
            notes: assignment_row.notes,
        })
    }

    /// Update a player's stack size (managers only)
    async fn update_stack_size(
        &self,
        ctx: &Context<'_>,
        input: UpdateStackSizeInput,
    ) -> Result<SeatAssignment> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let _manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let user_id = Uuid::parse_str(input.user_id.as_str()).gql_err("Invalid user ID")?;

        // Get current assignment for user
        let current_assignment =
            table_seat_assignments::get_current_for_user(&state.db, tournament_id, user_id)
                .await?
                .ok_or_else(|| {
                    async_graphql::Error::new("Player not currently assigned to a seat")
                })?;

        let update_data = UpdateSeatAssignment {
            stack_size: Some(input.new_stack_size),
            notes: None,
        };

        let assignment_row =
            table_seat_assignments::update(&state.db, current_assignment.id, update_data)
                .await?
                .ok_or_else(|| async_graphql::Error::new("Failed to update seat assignment"))?;

        // Get player info for the event
        let player = users::get_by_id(&state.db, user_id).await?;

        // Publish seating change event
        let club_id = get_club_id_for_tournament(&state.db, assignment_row.tournament_id).await?;
        let event = SeatingChangeEvent {
            event_type: SeatingEventType::StackUpdated,
            tournament_id: assignment_row.tournament_id.into(),
            club_id: club_id.into(),
            affected_assignment: Some(SeatAssignment {
                id: assignment_row.id.into(),
                tournament_id: assignment_row.tournament_id.into(),
                club_table_id: assignment_row.club_table_id.into(),
                user_id: assignment_row.user_id.into(),
                seat_number: assignment_row.seat_number,
                stack_size: assignment_row.stack_size,
                is_current: assignment_row.is_current,
                assigned_at: assignment_row.assigned_at,
                unassigned_at: assignment_row.unassigned_at,
                assigned_by: assignment_row.assigned_by.map(|id| id.into()),
                notes: assignment_row.notes.clone(),
            }),
            affected_player: player.map(|p| User {
                id: p.id.into(),
                email: p.email,
                username: p.username,
                first_name: p.first_name,
                last_name: p.last_name,
                phone: p.phone,
                is_active: p.is_active,
                role: Role::from(p.role),
            }),
            message: format!("Stack updated to {}", input.new_stack_size),
            timestamp: chrono::Utc::now(),
        };
        publish_seating_event(event);

        Ok(SeatAssignment {
            id: assignment_row.id.into(),
            tournament_id: assignment_row.tournament_id.into(),
            club_table_id: assignment_row.club_table_id.into(),
            user_id: assignment_row.user_id.into(),
            seat_number: assignment_row.seat_number,
            stack_size: assignment_row.stack_size,
            is_current: assignment_row.is_current,
            assigned_at: assignment_row.assigned_at,
            unassigned_at: assignment_row.unassigned_at,
            assigned_by: assignment_row.assigned_by.map(|id| id.into()),
            notes: assignment_row.notes,
        })
    }

    /// Automatically balance tables (managers only)
    async fn balance_tables(
        &self,
        ctx: &Context<'_>,
        input: BalanceTablesInput,
    ) -> Result<Vec<SeatAssignment>> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let tournament_id =
            Uuid::parse_str(input.tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Get all active tables (read before transaction)
        let tables = club_tables::list_assigned_to_tournament(&state.db, tournament_id).await?;
        if tables.is_empty() {
            return Ok(Vec::new());
        }

        // Begin transaction for all move operations
        let mut tx = state
            .db
            .begin()
            .await
            .gql_err("Database operation failed")?;

        // Get all current assignments within transaction
        let assignments =
            table_seat_assignments::list_current_for_tournament(&mut *tx, tournament_id)
                .await
                .gql_err("Database operation failed")?;

        // Group players by table and count
        let mut table_players: std::collections::HashMap<Uuid, Vec<_>> =
            std::collections::HashMap::new();
        for assignment in assignments {
            table_players
                .entry(assignment.club_table_id)
                .or_default()
                .push(assignment);
        }

        // Count players per table
        let mut table_counts: std::collections::HashMap<Uuid, usize> =
            std::collections::HashMap::new();
        for (table_id, players) in &table_players {
            table_counts.insert(*table_id, players.len());
        }

        // Check if rebalancing is needed
        if !needs_rebalancing(&table_counts) {
            return Ok(Vec::new()); // Tables are already balanced
        }

        let total_players = table_counts.values().sum::<usize>();
        let target_per_table = input
            .target_players_per_table
            .unwrap_or(((total_players as f64) / (tables.len() as f64)).ceil() as i32);

        // Create a plan for rebalancing
        let mut moves = Vec::new();

        // Find tables that need players and tables that have excess
        let mut need_players: Vec<_> = tables
            .iter()
            .filter(|table| {
                let current_count = table_players.get(&table.id).map(|v| v.len()).unwrap_or(0);
                current_count < target_per_table as usize
            })
            .collect();

        let mut excess_players: Vec<_> = Vec::new();
        for table in &tables {
            let empty_vec = Vec::new();
            let players = table_players.get(&table.id).unwrap_or(&empty_vec);
            if players.len() > target_per_table as usize {
                let excess_count = players.len() - target_per_table as usize;
                // Take the most recently assigned players for moving (they're likely less settled)
                let mut sorted_players = players.clone();
                sorted_players.sort_by(|a, b| b.assigned_at.cmp(&a.assigned_at));
                excess_players.extend(sorted_players.into_iter().take(excess_count));
            }
        }

        // Move excess players to tables that need them
        for player in excess_players {
            if let Some(target_table) = need_players.first() {
                let current_count = table_players
                    .get(&target_table.id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                if current_count < target_per_table as usize {
                    // Get all occupied seats in one query instead of checking each seat
                    let occupied_seats: std::collections::HashSet<i32> =
                        table_seat_assignments::get_occupied_seats(&mut *tx, target_table.id)
                            .await
                            .gql_err("Database operation failed")?
                            .into_iter()
                            .collect();

                    // Find the first available seat
                    let available_seat =
                        (1..=target_table.max_seats).find(|seat| !occupied_seats.contains(seat));

                    if let Some(seat_num) = available_seat {
                        // Unassign current seat and create new assignment
                        table_seat_assignments::unassign_current_seat(
                            &mut *tx,
                            tournament_id,
                            player.user_id,
                            Some(manager_id),
                        )
                        .await
                        .gql_err("Database operation failed")?;
                        let new_assignment = table_seat_assignments::create(
                            &mut *tx,
                            CreateSeatAssignment {
                                tournament_id,
                                club_table_id: target_table.id,
                                user_id: player.user_id,
                                seat_number: seat_num,
                                stack_size: player.stack_size,
                                assigned_by: Some(manager_id),
                                notes: Some("Balanced by system".to_string()),
                            },
                        )
                        .await
                        .gql_err("Database operation failed")?;

                        let assignment_for_response = SeatAssignment {
                            id: new_assignment.id.into(),
                            tournament_id: new_assignment.tournament_id.into(),
                            club_table_id: new_assignment.club_table_id.into(),
                            user_id: new_assignment.user_id.into(),
                            seat_number: new_assignment.seat_number,
                            stack_size: new_assignment.stack_size,
                            is_current: new_assignment.is_current,
                            assigned_at: new_assignment.assigned_at,
                            unassigned_at: new_assignment.unassigned_at,
                            assigned_by: new_assignment.assigned_by.map(|id| id.into()),
                            notes: new_assignment.notes.clone(),
                        };

                        moves.push(assignment_for_response);

                        // Update our tracking
                        table_players
                            .entry(target_table.id)
                            .or_default()
                            .push(new_assignment);

                        // Check if this table is now full
                        if table_players
                            .get(&target_table.id)
                            .map(|v| v.len())
                            .unwrap_or(0)
                            >= target_per_table as usize
                        {
                            need_players.remove(0);
                        }
                    }
                }
            }
        }

        // Commit transaction
        tx.commit().await.gql_err("Database operation failed")?;

        // Publish table balancing event after commit (side effects after successful commit)
        if !moves.is_empty() {
            let club_id = get_club_id_for_tournament(&state.db, tournament_id).await?;
            let event = SeatingChangeEvent {
                event_type: SeatingEventType::TablesBalanced,
                tournament_id: tournament_id.into(),
                club_id: club_id.into(),
                affected_assignment: None,
                affected_player: None,
                message: format!("{} players moved to balance tables", moves.len()),
                timestamp: chrono::Utc::now(),
            };
            publish_seating_event(event);
        }

        Ok(moves)
    }

    /// Eliminate a player from the tournament (managers only)
    async fn eliminate_player(
        &self,
        ctx: &Context<'_>,
        tournament_id: ID,
        user_id: ID,
        notes: Option<String>,
    ) -> Result<bool> {
        use crate::auth::permissions::require_role;

        // Require manager role
        let manager = require_role(ctx, Role::Manager).await?;

        let state = ctx.data::<AppState>()?;

        let tournament_uuid =
            Uuid::parse_str(tournament_id.as_str()).gql_err("Invalid tournament ID")?;
        let user_uuid = Uuid::parse_str(user_id.as_str()).gql_err("Invalid user ID")?;
        let manager_id = Uuid::parse_str(manager.id.as_str()).gql_err("Invalid manager ID")?;

        // Get current assignment for user
        let current_assignment =
            table_seat_assignments::get_current_for_user(&state.db, tournament_uuid, user_uuid)
                .await?;

        if let Some(assignment) = current_assignment {
            // Update the assignment with elimination notes and unassign
            let update_data = UpdateSeatAssignment {
                stack_size: Some(0), // Set stack to 0 to indicate elimination
                notes: notes
                    .clone()
                    .or_else(|| Some("Player eliminated".to_string())),
            };

            table_seat_assignments::update(&state.db, assignment.id, update_data).await?;
            table_seat_assignments::unassign(&state.db, assignment.id, Some(manager_id)).await?;

            // Get player info for the event
            let player = users::get_by_id(&state.db, user_uuid).await?;

            // Publish seating change event
            let club_id = get_club_id_for_tournament(&state.db, tournament_uuid).await?;
            let event = SeatingChangeEvent {
                event_type: SeatingEventType::PlayerEliminated,
                tournament_id: tournament_uuid.into(),
                club_id: club_id.into(),
                affected_assignment: Some(SeatAssignment {
                    id: assignment.id.into(),
                    tournament_id: assignment.tournament_id.into(),
                    club_table_id: assignment.club_table_id.into(),
                    user_id: assignment.user_id.into(),
                    seat_number: assignment.seat_number,
                    stack_size: Some(0),
                    is_current: false,
                    assigned_at: assignment.assigned_at,
                    unassigned_at: Some(chrono::Utc::now()),
                    assigned_by: assignment.assigned_by.map(|id| id.into()),
                    notes: notes.or_else(|| Some("Player eliminated".to_string())),
                }),
                affected_player: player.map(|p| User {
                    id: p.id.into(),
                    email: p.email,
                    username: p.username,
                    first_name: p.first_name,
                    last_name: p.last_name,
                    phone: p.phone,
                    is_active: p.is_active,
                    role: Role::from(p.role),
                }),
                message: "Player eliminated from tournament".to_string(),
                timestamp: chrono::Utc::now(),
            };
            publish_seating_event(event);

            Ok(true)
        } else {
            Err(async_graphql::Error::new(
                "Player not currently assigned to a seat",
            ))
        }
    }
}
