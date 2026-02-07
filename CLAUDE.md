# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PocketPair Service is a poker tournament management platform backend built with Rust. It provides a GraphQL API for managing poker clubs, tournaments, players, and real-time tournament features like clock management and table seating.

**Tech Stack:**
- Rust (Edition 2021)
- Axum 0.8 (web framework with WebSocket support)
- Async-GraphQL 7 (GraphQL server with subscriptions)
- SQLx 0.8 (compile-time verified SQL queries, PostgreSQL)
- Tokio 1.49 (async runtime)
- PostgreSQL 16

## Architecture

### Workspace Structure

This is a Cargo workspace with two crates:

- **`crates/api/`**: Main API application
  - GraphQL schema, queries, mutations, and subscriptions
  - REST routes for OAuth authentication
  - JWT middleware for authentication
  - Background services (tournament clock auto-advance, notifications)
  - Entry point: `src/main.rs`

- **`crates/infra/`**: Infrastructure/data layer
  - Database models (`models.rs`) - 18 `FromRow` structs
  - Repository pattern for data access (`repos/`) - 15 repositories
  - Database utilities (`db/`)
  - Pagination helpers
  - Scoring calculations (`scoring.rs`)

Note: `crates/telemetry/` and `crates/types/` directories exist but are empty stubs.

### Key Architectural Patterns

1. **Repository Pattern**: All database operations are in `crates/infra/src/repos/`. Each repository takes a `PgPool` clone and provides CRUD + domain-specific queries.

2. **GraphQL Layer** (`crates/api/src/gql/`):
   - `schema.rs` - Schema builder with DataLoaders
   - `queries.rs` - QueryRoot resolvers
   - `mutations.rs` - MutationRoot (50+ mutations)
   - `subscriptions.rs` - SubscriptionRoot (5 subscription endpoints)
   - `types.rs` - GraphQL type definitions and enums
   - `loaders.rs` - DataLoaders (ClubLoader, UserLoader, TournamentLoader)
   - `tournament_clock.rs` - Clock type and resolvers
   - `scalars.rs` - Custom scalar types

3. **State Management** (`crates/api/src/state.rs`):
   ```rust
   pub struct AppState {
       pub db: PgPool,
       jwt_service: JwtService,
       oauth_service: OAuthService,
   }
   ```

4. **Authentication & Authorization** (`crates/api/src/auth/`):
   - `jwt.rs` - JWT token creation/verification (Claims: sub, email, iat, exp)
   - `oauth.rs` - External OAuth provider integration (Google)
   - `custom_oauth.rs` - Custom OAuth server (username/password login)
   - `password.rs` - bcrypt password hashing
   - `permissions.rs` - Role-based access control (Admin, Manager, Player)
   - Permission helpers: `require_role()`, `require_admin()`, `require_club_manager()`, `require_admin_if()`

5. **Background Services** (`crates/api/src/services/`):
   - `clock_service.rs` - Checks every 5 seconds for tournament level advancement. Detects stale tournaments (24+ hours) every 5 minutes.
   - `notification_service.rs` - Sends "tournament starting soon" alerts.

6. **Real-time Features**:
   - GraphQL subscriptions over WebSocket
   - Uses Tokio broadcast channels (per-tournament, per-user, per-club) stored in static `Lazy<Arc<Mutex<HashMap>>>`
   - Publish functions: `publish_registration_event()`, `publish_seating_event()`, `publish_clock_update()`, `publish_user_notification()`
   - 5 subscription endpoints: clock updates, registrations, seating changes (per-tournament and per-club), user notifications

### Startup Flow (main.rs)

1. Initialize tracing (`RUST_LOG`, default: "info")
2. Load `.env` via dotenvy
3. Create PgPool (min 5, max 30 connections, 10s acquire timeout)
4. Run migrations from `../../migrations` (skip with `SKIP_MIGRATIONS=true`)
5. Create `AppState` (db + jwt + oauth)
6. Build GraphQL schema
7. Wait 2s for pool warmup
8. Spawn clock service and notification service
9. Build Axum router
10. Bind to `PORT` (default: 8080)

### HTTP Routes (app.rs)

```
GET  /health                        Health check with DB probe
GET  /auth/choose                   Unified auth choice page
GET  /auth/{provider}/authorize     External OAuth authorization
GET  /auth/{provider}/callback      External OAuth callback
GET  /oauth/authorize               Custom OAuth authorization
POST /oauth/login                   Custom OAuth login
POST /oauth/token                   Custom OAuth token
GET  /oauth/register                Custom OAuth registration form
POST /oauth/register                Custom OAuth registration
POST /graphql                       GraphQL queries/mutations
GET  /graphql                       GraphQL WebSocket subscriptions
```

**Middleware stack**: JWT extraction -> TraceLayer -> TimeoutLayer (30s) -> CorsLayer (permissive)

## Database

### Migrations

68 migration files in `./migrations/` (SQLx migration system). They run **automatically on startup** unless `SKIP_MIGRATIONS=true`.

- Custom ENUMs: `tournament_live_status`, `tournament_status`, `clock_status`
- Triggers auto-create tournament clocks, structures, and payouts
- Timestamp trigger auto-updates `updated_at` columns

### Key Entities

| Entity | Description |
|--------|-------------|
| `clubs` | Organizations hosting tournaments |
| `users` | Players/managers with roles (admin, manager, player) |
| `tournaments` | Events with lifecycle (not_started -> registration_open -> late_registration -> in_progress -> break -> final_table -> finished) |
| `tournament_clocks` | Real-time blind level state per tournament |
| `tournament_structure` | Blind level definitions (small/big blind, ante, duration) |
| `tournament_registrations` | Player registrations (registered, checked_in, seated, busted, waitlisted, cancelled, no_show) |
| `tournament_entries` | Buy-ins, rebuys, add-ons (amounts in integer cents) |
| `tournament_results` | Final positions and prize payouts |
| `tournament_payouts` | Prize pool distribution from templates |
| `club_tables` | Physical tables at a club |
| `table_seat_assignments` | Player-to-seat mappings with stack sizes |
| `club_managers` | Manager role assignments per club |
| `player_deals` | Side deals (even chop, ICM, custom) |
| `blind_structure_templates` | Reusable blind level templates |
| `payout_templates` | Reusable payout structures |

### Repositories (crates/infra/src/repos/)

`clubs`, `tournaments`, `users`, `tournament_registrations`, `tournament_results`, `tournament_clock`, `tournament_entries`, `tournament_payouts`, `table_seat_assignments`, `club_tables`, `club_managers`, `payout_templates`, `player_deals`, `blind_structure_templates`

## Development Commands

### Running Locally

```bash
# Run the API
cargo run --package api

# Check health
curl http://localhost:8080/health
```

### Building

```bash
# Build (offline mode for SQLx - required when DB is not available)
SQLX_OFFLINE=true cargo build --all-features

# Type check
SQLX_OFFLINE=true cargo check --all-features

# Clippy
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check
```

### Running Tests

Tests are in `crates/api/tests/` and use a real PostgreSQL database via `TEST_DATABASE_URL`.

```bash
# Run all tests
TEST_DATABASE_URL="..." cargo test --package api

# Run specific test file
TEST_DATABASE_URL="..." cargo test --package api --test tournament_tests

# Run specific test with output
TEST_DATABASE_URL="..." cargo test test_name --package api -- --nocapture
```

**Test files:** `auth_tests`, `tournament_tests`, `tournament_clock_tests`, `tournament_entries_tests`, `tournament_results_tests`, `permission_tests`, `table_seating_tests`, `club_tests`, `club_tables_test`, `user_tests`, `system_tests`, `integration_tests`, `payouts_tests`, `notification_tests`

**Test helpers** (`crates/api/tests/common/mod.rs`): `setup_test_db()`, `execute_graphql()`, `create_test_user()`, `create_test_club()`, `create_test_tournament()`, `create_club_manager()`, `create_test_club_table()`

### Database Management

```bash
# Run migrations manually
DATABASE_URL="..." cargo sqlx migrate run

# Revert last migration
DATABASE_URL="..." sqlx migrate revert

# Prepare SQLx metadata for offline builds (required after schema changes)
DATABASE_URL="..." cargo sqlx prepare --workspace -- --tests

# Reset database
DATABASE_URL="..." sqlx database reset -y
```

## Environment Variables

Set in `.env` for local development:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | (required) | PostgreSQL connection string |
| `TEST_DATABASE_URL` | `postgres://postgres:postgres@localhost:5432/pocketpair` | Test database |
| `PORT` | `8080` | Server port |
| `RUST_LOG` | `info` | Log level |
| `DATABASE_MAX_CONNECTIONS` | `30` | Connection pool max size |
| `SKIP_MIGRATIONS` | `false` | Skip auto-migrations on startup |
| `JWT_SECRET` | (required) | JWT signing secret |
| `JWT_EXPIRATION_HOURS` | `24` | Token lifetime |
| `GOOGLE_CLIENT_ID` | | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | | Google OAuth client secret |
| `REDIRECT_BASE_URL` | `http://localhost:8080` | OAuth redirect base URL |

**Note**: Special characters in database passwords must be URL-encoded (e.g., `?` -> `%3F`, `!` -> `%21`).

## GraphQL

The API is at `/graphql` (POST for queries/mutations, WebSocket for subscriptions).

### Authentication in Resolvers

```rust
// JWT claims injected by middleware into GraphQL context
let claims = ctx.data::<Claims>()?;

// Permission checks
let user = require_role(ctx, Role::Manager).await?;
let user = require_club_manager(ctx, club_id).await?;
let user = require_admin_if(ctx, condition, "field_name").await?;
```

### N+1 Prevention

```rust
let loader = ctx.data::<DataLoader<ClubLoader>>()?;
let club = loader.load_one(club_id).await?;
```

## Special Considerations

### SQLx Offline Mode

SQLx verifies queries at compile time. For builds without a live database:
1. Run migrations against a database
2. Generate metadata: `cargo sqlx prepare --workspace -- --tests`
3. Commit the `.sqlx/` directory
4. Build with `SQLX_OFFLINE=true`

### Tournament Clock System

Key files: `services/clock_service.rs`, `repos/tournament_clock.rs`, `gql/tournament_clock.rs`

The clock service runs every 5 seconds, advancing blind levels when `level_end_time` is reached. Stale tournaments (running 24+ hours) are auto-finished.

### Docker

Multi-stage Dockerfile using cargo-chef for dependency caching:
1. **Planner** - generates recipe.json
2. **Builder** - compiles release binary with `SQLX_OFFLINE=true` (Rust 1.92, Alpine 3.23)
3. **Runtime** - minimal Alpine with dumb-init, runs as non-root (uid 10001), health check on `/health`

### Scripts

- `scripts/dev.sh` - Development setup
- `scripts/migrate.sh` - Migration helpers
- `scripts/setup-pre-commit.sh` - Pre-commit hook setup
- `scripts/seed.sql` - Test data seeding
- `scripts/calculate_points.sql` - Points calculation SQL
