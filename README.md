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
- Use `sqlx migrate run` (inside the API container) to run migrations.
- Don’t commit `db_data/` — it’s in `.gitignore`.
