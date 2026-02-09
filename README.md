# PocketPair API

PocketPair is a poker tournament management platform backend built with Rust. The service provides a GraphQL API for managing poker clubs, tournaments, players, and real-time tournament features like clock management and table seating.

## Tech Stack

- **Rust** (Axum 0.8, Async-GraphQL, SQLx)
- **PostgreSQL 16**
- **Docker & docker-compose**

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
│   │   │   │   └── domains/         # Domain modules (see below)
│   │   │   │       ├── auth/        # OAuth, JWT, client management
│   │   │   │       ├── clubs/       # Club CRUD
│   │   │   │       ├── entries/     # Buy-ins, rebuys, add-ons
│   │   │   │       ├── leaderboards/# Scoring and rankings
│   │   │   │       ├── registrations/ # Registration, check-in (+ service)
│   │   │   │       ├── results/     # Positions, payouts, deals (+ service)
│   │   │   │       ├── seating/     # Table assignments, rebalancing (+ service)
│   │   │   │       ├── templates/   # Blind structure & payout templates
│   │   │   │       ├── tournaments/ # Tournament CRUD, clock management
│   │   │   │       └── users/       # Player CRUD
│   │   │   ├── auth/                # Authentication & authorization
│   │   │   ├── routes/              # REST routes (OAuth)
│   │   │   ├── middleware/          # JWT middleware
│   │   │   └── services/            # Background services (clock, notifications)
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
├── docker-compose.yml
├── Dockerfile
└── README.md
```

Each domain module under `domains/` contains:
- **`types.rs`** - GraphQL types, enums, inputs, and `From<Row>` impls
- **`resolvers.rs`** - Query and mutation resolvers (or `clock.rs` for tournaments)
- **`service.rs`** (optional) - Complex business logic extracted from resolvers (registrations, seating, results)

---

## Architecture

### Key Patterns

1. **Domain-Based GraphQL Layer**: Each domain (auth, clubs, tournaments, etc.) has its own module with types, resolvers, and optional services. `gql/types.rs` is a barrel re-export file so all imports work unchanged.

2. **Repository Pattern**: All database operations in `crates/infra/src/repos/`. Each repository provides CRUD and domain-specific queries.

3. **Domain Services**: Complex mutations (check-in, table balancing, results) are extracted into `service.rs` files that own transactions and return infra Row types. Resolvers handle auth, ID parsing, type conversions, and event publishing.

4. **Type Conversions**: Each domain's `types.rs` includes `From<Row>` impls (e.g., `From<TournamentRow> for Tournament`) for clean `.into()` conversions.

5. **Club-Scoped Authorization**: Most mutations use `require_club_manager(ctx, club_id)` so managers can only act on their own clubs. Three roles: Admin, Manager, Player.

6. **JWT Authentication**: Middleware validates tokens and injects claims into GraphQL context.

7. **Background Services**: Tournament clock service auto-advances blind levels every 5 seconds. Notification service sends pre-tournament alerts.

8. **Real-time Updates**: GraphQL subscriptions over WebSocket for live tournament data (clock, seating, registrations, notifications).

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

- Docker & docker-compose
- Rust (optional, for local development without Docker)

### Environment Configuration

Create a `.env` file:

```env
DATABASE_URL="postgres://pocketpair:pocketpair@localhost:5432/pocketpair"
PG_DB=pocketpair
PG_USER=pocketpair
PG_PASSWORD=pocketpair
RUST_LOG=info
```

### Start Services

```bash
# Start PostgreSQL and API
docker compose up -d --build

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

Tests are in `crates/api/tests/` and require a test database:

```bash
# Run all tests
TEST_DATABASE_URL="postgres://user:pass@localhost:5432/pocketpair" \
  cargo test --package api

# Run specific test file
TEST_DATABASE_URL="..." cargo test --package api --test tournament_tests

# Run with output
TEST_DATABASE_URL="..." cargo test --package api -- --nocapture
```

**Test files** (`crates/api/tests/`):
- `auth_tests` - Authentication and JWT
- `tournament_tests` - Tournament CRUD and lifecycle
- `tournament_clock_tests` - Clock state and level advancement
- `clock_lifecycle_tests` - Full clock lifecycle scenarios
- `tournament_entries_tests` - Buy-ins, rebuys, add-ons
- `tournament_results_tests` - Results and leaderboard
- `payouts_tests` - Prize pool and payouts
- `notification_tests` - Real-time notifications
- `permission_tests` - Role-based access control
- `table_seating_tests` - Table and seat management
- `check_in_tests` - Player check-in with auto-seating
- `eliminate_player_tests` - Player elimination flow
- `club_tests`, `club_tables_test` - Club CRUD and tables
- `user_tests`, `player_management_tests` - User/player management
- `unassign_table_tests` - Table unassignment
- `query_coverage_tests` - GraphQL query coverage
- `system_tests` - System-level integration

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

### LeaderboardPeriod
`ALL_TIME`, `LAST_YEAR`, `LAST_6_MONTHS`, `LAST_30_DAYS`, `LAST_7_DAYS`

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | required |
| `TEST_DATABASE_URL` | Test database connection | - |
| `RUST_LOG` | Logging level | `info` |
| `PORT` | Server port | `8080` |
| `DATABASE_MAX_CONNECTIONS` | Connection pool size | `30` |
| `SKIP_MIGRATIONS` | Skip auto-migrations | `false` |

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

```yaml
version: '3.8'
services:
  api:
    image: ghcr.io/brunomoyse/pp-service:latest
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgres://user:pass@postgres:5432/pocketpair
      - RUST_LOG=info
    depends_on:
      postgres:
        condition: service_healthy

  postgres:
    image: postgres:16-alpine
    environment:
      - POSTGRES_DB=pocketpair
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=pass
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U user -d pocketpair"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  postgres_data:
```

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
