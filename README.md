# PocketPair API

PocketPair is a poker tournament management platform backend built with Rust. The service provides a GraphQL API for managing poker clubs, tournaments, players, and real-time tournament features like clock management and table seating.

## Tech Stack

- **Rust** (Axum 0.8, Async-GraphQL 7, SQLx 0.8, Tokio)
- **PostgreSQL 16** (custom ENUMs, triggers, `LISTEN`/`NOTIFY`)
- **Docker & docker-compose** (also used by the testcontainers-based integration suite)

---

## Project Structure

```
.
├── crates/
│   ├── api/                         # Main API application
│   │   ├── src/
│   │   │   ├── main.rs              # Entry point
│   │   │   ├── app.rs               # Router & middleware setup
│   │   │   ├── state.rs             # Shared application state (PgPool, JWT, OAuth)
│   │   │   ├── gql/                 # GraphQL layer
│   │   │   │   ├── schema.rs        # Schema builder with DataLoaders
│   │   │   │   ├── root/            # Query & mutation root resolvers
│   │   │   │   ├── subscriptions.rs # Real-time subscriptions
│   │   │   │   ├── types.rs         # Barrel re-export (all types in domains/)
│   │   │   │   ├── loaders.rs       # DataLoaders (N+1 prevention)
│   │   │   │   ├── common/          # Shared types (Role, notifications), helpers
│   │   │   │   └── domains/         # ~30 domain modules (see below)
│   │   │   │       ├── auth/        # OAuth, JWT, refresh tokens
│   │   │   │       ├── clubs/       # Club CRUD
│   │   │   │       ├── tournaments/ # Tournament CRUD, clock, recurrence
│   │   │   │       ├── registrations/ # Registration, check-in (+ service)
│   │   │   │       ├── seating/     # Table assignments, rebalancing (+ service)
│   │   │   │       ├── entries/     # Buy-ins, rebuys, add-ons
│   │   │   │       ├── results/     # Positions, payouts, deals (+ service)
│   │   │   │       ├── templates/   # Blind structure & payout templates
│   │   │   │       ├── series/      # Multi-day flights (one event, many days)
│   │   │   │       ├── leaderboards/ & leaderboard_configs/  # Rankings & leagues
│   │   │   │       ├── achievements/, drinks/, predictions/, social/, ...
│   │   │   │       └── users/       # Player CRUD
│   │   │   ├── auth/                # Authentication & authorization
│   │   │   ├── routes/              # REST routes (OAuth)
│   │   │   ├── middleware/          # JWT middleware
│   │   │   └── services/            # Background services (clock, notifications, …)
│   │   └── tests/                   # Integration tests
│   │
│   └── infra/                       # Infrastructure/data layer
│       └── src/
│           ├── models.rs            # Database models (FromRow structs)
│           ├── repos/               # Repository pattern
│           └── db/                  # Database utilities
│
├── migrations/                      # SQLx database migrations
├── .sqlx/                           # SQLx offline query metadata
├── deploy/                          # Production Compose + backup tooling
├── justfile                         # Common dev/test/db recipes
├── Dockerfile                       # (Compose lives at the monorepo root)
└── README.md
```

Each domain module under `domains/` contains:
- **`types.rs`** - GraphQL types, enums, inputs, and `From<Row>` impls
- **`resolvers.rs`** - Query and mutation resolvers (or `clock.rs` for tournaments)
- **`service.rs`** (optional) - Complex business logic extracted from resolvers (registrations, seating, results)

Beyond the core poker domains listed above, the schema also covers engagement and
platform features in their own modules: `achievements`, `activity_log`,
`announcements`, `attendance`, `analytics`, `cosmetics`, `devices` (push tokens),
`drinks` (bar credits/vouchers), `identity`, `notes`, `predictions`, `pro`,
`scouting`, `seasons`, `social`, and `system`.

---

## Architecture

### Key Patterns

1. **Domain-Based GraphQL Layer**: Each domain (auth, clubs, tournaments, etc.) has its own module with types, resolvers, and optional services. `gql/types.rs` is a barrel re-export file so all imports work unchanged.

2. **Repository Pattern**: All database operations in `crates/infra/src/repos/`. Each repository provides CRUD and domain-specific queries.

3. **Domain Services**: Complex mutations (check-in, table balancing, results) are extracted into `service.rs` files that own transactions and return infra Row types. Resolvers handle auth, ID parsing, type conversions, and event publishing.

4. **Type Conversions**: Each domain's `types.rs` includes `From<Row>` impls (e.g., `From<TournamentRow> for Tournament`) for clean `.into()` conversions.

5. **Club-Scoped Authorization**: Most mutations use `require_club_manager(ctx, club_id)` so managers can only act on their own clubs. Three roles: Admin, Manager, Player.

6. **JWT Authentication**: Middleware validates tokens and injects claims into GraphQL context.

7. **Background Services** (`services/`): the clock service auto-advances blind levels every 5 seconds (and auto-finishes stale tournaments); the notification service sends pre-tournament alerts; the drink-expiry service expires bar credits; the data-retention service anonymizes dormant player accounts (off unless `ENABLE_DATA_RETENTION=true`). Email and push delivery degrade gracefully when unconfigured.

8. **Real-time Updates**: GraphQL subscriptions over WebSocket for live tournament data (clock, seating, registrations, activity, notifications). Per-instance fan-out uses Tokio broadcast channels; cross-instance fan-out uses **Postgres `LISTEN`/`NOTIFY`**, so the backend can run more than one replica.

9. **Recurring Tournaments**: `createTournament` accepts an optional `recurrenceFrequency` (`WEEKLY`/`MONTHLY`) + `recurrenceEndDate`. When set, the resolver expands the spec into independent tournament occurrences and creates them in a single transaction, copying the blind structure to each.

### Database Concepts

- **Clubs**: Organizations that host tournaments
- **Tournaments**: Events with lifecycle (not_started → registration_open → in_progress → finished)
- **Tournament Clocks**: Manages blind levels and timing
- **Tournament Structure**: Defines blind levels
- **Club Tables**: Physical tables assigned to tournaments
- **Seat Assignments**: Player seating arrangements
- **Club Managers**: Users with manager role for specific clubs

---

## Getting Started

### Prerequisites

- Docker & docker-compose (also required to run the test suite)
- Rust (optional, for local development without Docker)

### Environment Configuration

Copy the template and fill it in — `.env.example` documents every variable:

```bash
cp .env.example .env
```

At minimum set a database URL and a secure `JWT_SECRET` (the only hard-required
secret):

```env
# Local Docker Compose maps Postgres to host port 15432
DATABASE_URL=postgres://postgres:admin@localhost:15432/pocketpair
JWT_SECRET=$(openssl rand -base64 32)
RUST_LOG=info
```

Google OAuth (`GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`), email (`SCW_*`), push
(`EXPO_ACCESS_TOKEN`) and AI roster import (`OPENROUTER_API_KEY`) are all optional
— each feature degrades gracefully when its variables are absent.

### Start Services

The `docker-compose.yml` that orchestrates Postgres + this backend lives at the
**monorepo root**, so run Compose from there:

```bash
# From the repository root
docker compose up -d --build   # Postgres (host port 15432) + backend (8080)

# Check health
curl http://localhost:8080/health
```

---

## Development

### Running Locally

```bash
# With cargo
cargo run --package api

# Build (offline mode for SQLx)
SQLX_OFFLINE=true cargo build --all-features

# Type check
SQLX_OFFLINE=true cargo check --all-features

# Linting
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check
```

### Running Tests

Integration tests live in `crates/api/tests/integration/` and compile into a
single `integration` test binary. They spin up their **own** throwaway
PostgreSQL 16 container (via `testcontainers`) and run all migrations against it,
so **no external database is required — only a running Docker daemon**. Each test
gets its own pool against the shared container.

```bash
# Run everything (unit + integration). SQLX_OFFLINE avoids needing a build-time DB.
SQLX_OFFLINE=true cargo test --package api

# Just the integration binary
SQLX_OFFLINE=true cargo test --package api --test integration

# A single module or test, with output
SQLX_OFFLINE=true cargo test --package api --test integration tournament -- --nocapture

# Remove a leftover test container from an interrupted run
just cleanup-testcontainers
```

Each `mod` in `crates/api/tests/integration/main.rs` is one suite — e.g.
`tournament`, `tournament_clock`, `check_in`, `table_seating`, `payouts`,
`permission`, `authz_guards`, `economy_firewall`, `money_reconciliation`,
`data_retention`, `drinks`, and more. Pure logic (e.g. recurrence date math) is
covered by `#[cfg(test)]` unit tests inside the relevant module.

### Database Management

Migrations run automatically on startup. Manual commands:

```bash
# Run migrations
DATABASE_URL="..." cargo sqlx migrate run

# Revert last migration
DATABASE_URL="..." sqlx migrate revert

# Reset database
DATABASE_URL="..." sqlx database reset -y

# Prepare SQLx metadata (after schema changes)
DATABASE_URL="..." cargo sqlx prepare --workspace -- --tests
```

Common workflows are also wrapped in the `justfile`: `just migrate`, `just seed`,
`just db-reset`, `just sqlx-prepare`, and `just cleanup-testcontainers`.

---

## GraphQL API

The GraphQL endpoint is available at `/graphql` (POST for queries/mutations, WebSocket for subscriptions).

### Authentication

Include JWT token in requests:
```
Authorization: Bearer <your-jwt-token>
```

### Key Queries

| Query | Description |
|-------|-------------|
| `tournaments` | List tournaments with filters |
| `tournament(id)` | Get tournament details |
| `tournamentClock(tournamentId)` | Get clock state |
| `tournamentPlayers(tournamentId)` | Get registered players |
| `tournamentSeatingChart(tournamentId)` | Get seating arrangement |
| `tournamentPayout(tournamentId)` | Get payout structure |
| `clubs` | List all clubs |
| `me` | Get authenticated user |
| `leaderboard(period, clubId)` | Get player rankings |
| `myTournamentStatistics` | Get personal stats |

### Key Mutations

| Mutation | Description | Role |
|----------|-------------|------|
| `createTournament` | Create a tournament (optionally recurring) | Manager |
| `updateTournament` | Edit tournament details | Manager |
| `assignTablesToTournament` | Link physical tables | Manager |
| `createTournamentClock` | Initialize clock | Manager |
| `startTournamentClock` | Start clock | Manager |
| `pauseTournamentClock` | Pause clock | Manager |
| `advanceTournamentLevel` | Next blind level | Manager |
| `updateTournamentStatus` | Change live status | Manager |
| `registerForTournament` | Player registration | Any |
| `checkInPlayer` | Check in with auto-seat | Manager |
| `assignPlayerToSeat` | Manual seating | Manager |
| `movePlayer` | Move to different seat | Manager |
| `eliminatePlayer` | Remove from tournament | Manager |
| `addTournamentEntry` | Add buy-in/rebuy/addon | Manager |
| `enterTournamentResults` | Record final results | Manager |

### Subscriptions

| Subscription | Description |
|--------------|-------------|
| `tournamentClockUpdates(tournamentId)` | Real-time clock updates |
| `tournamentRegistrations(tournamentId)` | Registration events |
| `tournamentSeatingChanges(tournamentId)` | Seating updates |
| `userNotifications` | Personal notifications |

### Example Queries

```graphql
# Get tournament with players
query GetTournament($id: UUID!) {
  tournament(id: $id) {
    id
    title
    liveStatus
    buyInCents
    club { name }
  }
  tournamentPlayers(tournamentId: $id) {
    user { username firstName lastName }
    registration { status }
  }
}

# Subscribe to clock updates
subscription ClockUpdates($id: ID!) {
  tournamentClockUpdates(tournamentId: $id) {
    currentLevel
    timeRemainingSeconds
    status
    smallBlind
    bigBlind
  }
}
```

---

## Enums

### TournamentLiveStatus
`NOT_STARTED` → `REGISTRATION_OPEN` → `LATE_REGISTRATION` → `IN_PROGRESS` → `BREAK` → `FINAL_TABLE` → `FINISHED`

### RegistrationStatus
`REGISTERED`, `CHECKED_IN`, `SEATED`, `WAITLISTED`, `CANCELLED`, `NO_SHOW`, `BUSTED`

### Role
`ADMIN`, `MANAGER`, `PLAYER`

### EntryType
`INITIAL`, `REBUY`, `RE_ENTRY`, `ADDON`

### BountyType
`NONE`, `FIXED`, `PROGRESSIVE`

### RecurrenceFrequency
`WEEKLY`, `MONTHLY`

### LeaderboardPeriod
`ALL_TIME`, `LAST_YEAR`, `LAST_6_MONTHS`, `LAST_30_DAYS`, `LAST_7_DAYS`

---

## Environment Variables

See `.env.example` for the complete, commented list. The most important:

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | **required** |
| `JWT_SECRET` | Secret for signing JWTs (`openssl rand -base64 32`) | **required** |
| `RUST_LOG` | Logging level | `info` |
| `PORT` | Server port | `8080` |
| `DATABASE_MAX_CONNECTIONS` | Connection pool size | `30` |
| `SKIP_MIGRATIONS` | Skip auto-migrations on startup | `false` |
| `JWT_EXPIRATION_HOURS` | Access-token lifetime | `24` |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | Google OAuth (optional) | - |
| `REDIRECT_BASE_URL` | OAuth callback base URL | `http://localhost:8080` |
| `ALLOWED_ORIGINS` | CORS allowlist (production) | - |
| `COOKIE_PATH` / `COOKIE_DOMAIN` / `COOKIE_SECURE` | Refresh-cookie scoping (set `COOKIE_PATH=/api/auth` behind a `/api` proxy) | - |
| `GQL_INTROSPECTION` | Allow schema introspection | `true` |
| `GQL_QUERY_DEPTH_LIMIT` / `GQL_QUERY_COMPLEXITY_LIMIT` | Query guards | `15` / `200` |
| `ENABLE_DATA_RETENTION` | Anonymize dormant player accounts | `false` |
| `SCW_*` | Scaleway transactional email (optional) | - |
| `EXPO_ACCESS_TOKEN` | Expo push notifications (optional) | - |
| `OPENROUTER_API_KEY` | AI-assisted roster import (optional) | - |

**Note**: Special characters in passwords must be URL-encoded (e.g., `?` → `%3F`, `!` → `%21`)

---

## Docker Deployment

### Production Images

Images are built on push to `main` and published to GitHub Container Registry:

```bash
# Pull latest
docker pull ghcr.io/brunomoyse/pp-service:latest

# Run
docker run -d \
  --name pocketpair-api \
  -p 8080:8080 \
  -e DATABASE_URL="postgres://user:pass@host:5432/pocketpair" \
  ghcr.io/brunomoyse/pp-service:latest
```

### Docker Compose (Production)

A production-oriented Compose file with resource limits and automated backups is
provided at [`deploy/docker-compose.prod.example.yml`](deploy/docker-compose.prod.example.yml)
(copy `deploy/.env.production.example` to `deploy/.env.production` and fill it in;
see `deploy/README.md`). Adapt it to your host rather than copying a snippet; run
it from the `deploy/` directory:

```bash
docker compose -f docker-compose.prod.example.yml --env-file .env.production up -d
```

The repo-root `docker-compose.yml` orchestrates Postgres + backend for local
development (Postgres on host port 15432, API on 8080).

---

## SQLx Offline Mode

SQLx requires compile-time query verification. For CI/Docker builds:

1. Make schema changes and run migrations
2. Run `cargo sqlx prepare --workspace -- --tests`
3. Commit `.sqlx/` directory
4. Build with `SQLX_OFFLINE=true`

---

## License

Private repository - all rights reserved.
