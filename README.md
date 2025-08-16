# PocketPair API

PocketPair is an app designed to let poker players easily track tournament schedules, results, and updates, while helping poker clubs organize and manage game data efficiently.
This repository is the API side: a Rust + Axum + Async-GraphQL backend service with a PostgreSQL database.  

## Tech Stack

- **Rust** (Axum 0.8, Async-GraphQL, SQLx)
- **PostgreSQL** (v16, running in Docker)
- **Docker & docker-compose** for local development

---

## Project Structure

```
.
├── crates/api/          # API crate
│   ├── src/
│   │   ├── app.rs       # Router & middleware
│   │   ├── state.rs     # Shared application state
│   │   ├── error.rs     # Error handling
│   │   ├── gql/         # GraphQL schema & resolvers
│   │   ├── routes/      # REST routes
│   │   └── middleware/  # Custom middleware
│   └── main.rs          # Entry point
├── config/              # Environment configs (default/local/prod)
├── migrations/          # SQLx migrations
├── docker-compose.yml
├── Dockerfile
└── README.md
```

---

## Getting Started

### 1. Prerequisites
- Docker & docker-compose
- Rust (optional, if running without Docker)

### 2. Configure Environment

Create a `.env` file in the project root:
```env
PG_DB=pocketpair
PG_USER=pocketpair
PG_PASSWORD=pocketpair
RUST_LOG=info
```

### 3. Start Services

```bash
docker compose up -d --build
```

- API: [http://localhost:8080](http://localhost:8080)

---

## Health Check
```bash
curl http://localhost:8080/health
```

---

## Development Notes
- Postgres data persists in `./db_data` (bind-mounted from host).
- **Migrations run automatically** on app startup (see Migration section below).
- Don't commit `db_data/` — it's in `.gitignore`.

---

## Database Migrations

### Automatic Migrations
The application **automatically runs database migrations on startup**. This ensures your database schema is always up-to-date when the app starts.

**Environment Variables:**
- `SKIP_MIGRATIONS=true` - Skip automatic migrations (useful for read-only replicas)

### Manual Migration Commands
If needed, you can still run migrations manually:

```bash
# Inside the API container
sqlx migrate run

# Or using docker-compose
docker-compose exec api sqlx migrate run
```

### Migration Files
Migration files are located in `./migrations/` and follow the pattern:
- `YYYYMMDDHHMMSS_description.up.sql` - Apply migration
- `YYYYMMDDHHMMSS_description.down.sql` - Rollback migration

---

## Docker Deployment

### Production Docker Images

Docker images are automatically built and pushed to GitHub Container Registry (GHCR) when changes are pushed to the `main` branch.

**Available Images:**
- `ghcr.io/brunomoyse/pp-service:latest` - Latest production build
- `ghcr.io/brunomoyse/pp-service:main-<sha>` - Specific commit builds

### Using the Production Image

```bash
# Pull the latest image
docker pull ghcr.io/brunomoyse/pp-service:latest

# Run with environment variables (migrations will run automatically)
docker run -d \
  --name pocketpair-api \
  -p 8080:8080 \
  -e DATABASE_URL="postgres://user:pass@host:5432/pocketpair" \
  -e RUST_LOG=info \
  ghcr.io/brunomoyse/pp-service:latest
```

### Docker Compose for Production

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
      # - SKIP_MIGRATIONS=true  # Uncomment to skip auto-migrations
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

### GitHub Actions Workflows

Two Docker build workflows are available:

1. **docker-build.yml** - Builds after CI passes (recommended for production)
2. **docker-build-simple.yml.example** - Builds immediately on push (rename to `.yml` for development)
