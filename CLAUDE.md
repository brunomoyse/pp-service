# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PocketPair is a poker tournament management platform backend built with Rust. The service provides a GraphQL API for managing poker clubs, tournaments, players, and real-time tournament features like clock management and table seating.

**Tech Stack:**
- Rust (Axum 0.8, Async-GraphQL, SQLx)
- PostgreSQL 16
- Docker & docker-compose
- Prometheus + Grafana (monitoring)

## Architecture

### Workspace Structure

This is a Cargo workspace with two crates:

- **`crates/api/`**: Main API application
  - GraphQL schema, queries, mutations, and subscriptions
  - REST routes for OAuth authentication
  - JWT middleware for authentication
  - Background services (tournament clock auto-advance)
  - Prometheus metrics (`src/metrics/`)
  - Entry point: `src/main.rs`

- **`crates/infra/`**: Infrastructure/data layer
  - Database models (`models.rs`)
  - Repository pattern for data access (`repos/`)
  - Database utilities and notifications (`db/`)
  - Pagination helpers
  - Scoring calculations

### Key Architectural Patterns

1. **Repository Pattern**: All database operations are encapsulated in repository structs in `crates/infra/src/repos/`. Each repository (e.g., `TournamentRepo`, `UserRepo`) provides methods for CRUD operations and domain-specific queries.

2. **GraphQL Layer**: The API exposes a GraphQL schema built with async-graphql:
   - Schema definition: `crates/api/src/gql/schema.rs`
   - Queries: `crates/api/src/gql/queries.rs`
   - Mutations: `crates/api/src/gql/mutations.rs`
   - Subscriptions: `crates/api/src/gql/subscriptions.rs`
   - Types: `crates/api/src/gql/types.rs`
   - DataLoaders: `crates/api/src/gql/loaders.rs` (N+1 query prevention)

3. **State Management**: `AppState` (in `crates/api/src/state.rs`) holds shared application state including:
   - Database connection pool (`PgPool`)
   - JWT service for token generation/validation
   - OAuth service for third-party authentication

4. **Authentication & Authorization**:
   - JWT-based authentication middleware in `crates/api/src/middleware/jwt.rs`
   - Role-based permissions (Admin, Manager, Player) in `crates/api/src/auth/permissions.rs`
   - OAuth flows for external providers + custom OAuth server
   - Permission helpers: `require_role()`, `require_club_manager()`, `require_admin_if()`

5. **Background Services**:
   - Tournament clock service (`crates/api/src/services/clock_service.rs`) runs in background
   - Auto-advances tournament levels every 5 seconds when enabled
   - Uses PostgreSQL NOTIFY for real-time updates

6. **Real-time Features**:
   - GraphQL subscriptions for live tournament updates
   - PostgreSQL LISTEN/NOTIFY for event broadcasting (see `crates/infra/src/db/notifications.rs`)

7. **Metrics & Monitoring**:
   - Prometheus metrics exposed on port 9090 at `/metrics`
   - HTTP request metrics middleware (`crates/api/src/middleware/metrics.rs`)
   - Business metrics organized by domain (`crates/api/src/metrics/mod.rs`):
     - `tournament` - tournament creation, status transitions, durations
     - `graphql` - request counts, durations, errors
     - `db` - query durations and errors
     - `websocket` - subscription counts, messages sent
     - `clock_service` - tick durations, auto-advances
     - `auth` - OAuth and JWT operations
   - Database connection pool monitoring (active/idle connections)

## Database

### Migrations

Migrations are in `./migrations/` and use SQLx's migration system. They run **automatically on application startup** unless `SKIP_MIGRATIONS=true` is set.

**Important migration notes:**
- The database uses custom ENUMs (e.g., `tournament_live_status`, `tournament_status`, `clock_status`)
- Database triggers auto-create related records (tournament clocks, tournament structures, tournament payouts)
- The schema includes a timestamp trigger that automatically updates `updated_at` columns

### Key Database Concepts

- **Clubs**: Organizations that host tournaments
- **Tournaments**: Events with lifecycle tracked by `live_status` (not_started → registration_open → in_progress → finished)
- **Tournament Clocks**: Manages blind levels and timing for each tournament
- **Tournament Structure**: Defines blind levels for tournaments
- **Club Tables**: Physical tables assigned to tournaments
- **Table Seat Assignments**: Player seating arrangements
- **Club Managers**: Users with manager role for specific clubs (separate from global Admin role)

### Database Connection

The main application connects using `DATABASE_URL` environment variable. Tests use `TEST_DATABASE_URL` (defaults to `postgres://postgres:postgres@localhost:5432/pocketpair` if not set).

Production database credentials are URL-encoded in environment variables to handle special characters.

## Development Commands

### Local Development

```bash
# Start PostgreSQL and API in Docker
docker compose up -d --build

# Check health
curl http://localhost:8080/health

# Check metrics
curl http://localhost:9090/metrics
```

### Monitoring Stack

```bash
# Start with Prometheus + Grafana monitoring
docker compose -f docker-compose.monitoring.yml up -d --build

# Access points:
# - API: http://localhost:8080
# - Prometheus: http://localhost:9091
# - Grafana: http://localhost:3000 (admin/admin)
# - Metrics: http://localhost:9090/metrics
```

### Running Tests

Tests are located in `crates/api/tests/` and use a test database.

```bash
# Run all integration tests
TEST_DATABASE_URL="postgres://brunomoyse:A9WeJQk%3F%217W0n%C2%A3h%C2%A3@192.168.0.14:5433/pocketpair" cargo test --package api --test integration_tests

# Run specific test file
TEST_DATABASE_URL="..." cargo test --package api --test tournament_tests

# Run specific test
TEST_DATABASE_URL="..." cargo test test_me_query_unauthenticated --package api

# Run with output visible
TEST_DATABASE_URL="..." cargo test --package api --test tournament_tests -- --nocapture
```

**Test helpers** are in `crates/api/tests/common/mod.rs`:
- `setup_test_db()`: Initialize test database connection
- `execute_graphql()`: Execute GraphQL queries/mutations with optional auth
- `create_test_user()`: Create user and return JWT claims
- `create_test_club()`: Create test club
- `create_test_tournament()`: Create test tournament
- `create_club_manager()`: Assign manager to club

### Building and Type Checking

```bash
# Build (offline mode for SQLx)
SQLX_OFFLINE=true cargo build --all-features

# Type check
SQLX_OFFLINE=true cargo check --all-features

# Run clippy
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings

# Format check
SQLX_OFFLINE=true cargo fmt --all -- --check
```

### Database Management

```bash
# Run migrations manually
DATABASE_URL="..." cargo sqlx migrate run

# Revert last migration
DATABASE_URL="..." sqlx migrate revert

# View migration status
DATABASE_URL="..." sqlx migrate info

# Reset database (drop + recreate + migrate)
DATABASE_URL="..." sqlx database reset -y

# Prepare SQLx metadata for offline builds (required after schema changes)
DATABASE_URL="..." cargo sqlx prepare --workspace -- --tests
```

### Running the API Locally

```bash
# With cargo
cargo run --package api

# With timeout (useful for testing)
timeout 5 cargo run --package api
```

## Environment Variables

Required environment variables (set in `.env` for local development):

- `DATABASE_URL`: PostgreSQL connection string
- `TEST_DATABASE_URL`: Test database connection string (for running tests)
- `RUST_LOG`: Logging level (default: `info`)
- `PORT`: Server port (default: `8080`)
- `DATABASE_MAX_CONNECTIONS`: Connection pool size (default: `30`)
- `SKIP_MIGRATIONS`: Skip auto-migrations on startup (default: `false`)
- OAuth configuration (see `crates/api/src/auth/config.rs`)

**Production URL encoding note**: Special characters in passwords must be URL-encoded (e.g., `?` → `%3F`, `!` → `%21`, `£` → `%C2%A3`)

## GraphQL Schema

The GraphQL API is available at `/graphql` (both POST for queries/mutations and WebSocket for subscriptions).

### Authentication in GraphQL

- JWT claims are injected into GraphQL context by `jwt_middleware` and custom `graphql_handler`
- Access claims in resolvers: `ctx.data::<Claims>()?`
- Use permission helpers from `crates/api/src/auth/permissions.rs`

### Common Patterns

**N+1 Query Prevention**: Use DataLoaders (see `crates/api/src/gql/loaders.rs`):
```rust
let club_loader = ctx.data::<DataLoader<ClubLoader>>()?;
let club = club_loader.load_one(club_id).await?;
```

**Permission Checks**:
```rust
// Require specific role
let user = require_role(ctx, Role::Manager).await?;

// Require club manager (or admin)
let user = require_club_manager(ctx, club_id).await?;

// Conditionally require admin
let user = require_admin_if(ctx, is_changing_sensitive_field, "field_name").await?;
```

## Testing Strategy

- Integration tests use real PostgreSQL database (via `TEST_DATABASE_URL`)
- Tests are organized by feature in `crates/api/tests/`:
  - `auth_tests.rs` - authentication and JWT tests
  - `tournament_tests.rs` - tournament CRUD and lifecycle
  - `tournament_clock_tests.rs` - clock state and level advancement
  - `permission_tests.rs` - role-based access control
  - `table_seating_tests.rs` - table and seat assignment
  - `club_tests.rs`, `club_tables_test.rs` - club management
  - `user_tests.rs` - user operations
  - `system_tests.rs` - end-to-end system tests
  - `integration_tests.rs` - general integration tests
- Use test helpers from `common/mod.rs` for setup
- Clean test data is ensured through database transactions or cleanup

## Special Considerations

### Tournament Clock System

The tournament clock auto-advances levels based on time. Key files:
- Service: `crates/api/src/services/clock_service.rs`
- Repository: `crates/infra/src/repos/tournament_clock.rs`
- GraphQL: `crates/api/src/gql/tournament_clock.rs`

The service runs every 5 seconds and checks for tournaments ready to advance.

### SQLx Offline Mode

SQLx requires compile-time query verification. For offline builds (e.g., in Docker):
1. Make schema changes and run migrations
2. Run `cargo sqlx prepare --workspace -- --tests` to generate metadata
3. Commit `.sqlx/` directory
4. Build with `SQLX_OFFLINE=true`

### URL Encoding in Database URLs

Production database passwords with special characters MUST be URL-encoded when used in `DATABASE_URL` or `TEST_DATABASE_URL`.

### Metrics Implementation

The metrics system uses the `metrics` crate ecosystem:
- `metrics` - Core metrics facade
- `metrics-exporter-prometheus` - Prometheus exporter
- `metrics-util` - Utilities for idle timeout

To add new metrics:
```rust
use metrics::{counter, gauge, histogram};

// Counter - monotonically increasing value
counter!("my_events_total", "label" => "value").increment(1);

// Gauge - value that can go up and down
gauge!("my_current_value").set(42.0);

// Histogram - track distribution of values
histogram!("my_duration_seconds").record(0.5);
```
