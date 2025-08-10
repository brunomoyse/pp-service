
#!/usr/bin/env bash
set -euo pipefail

# Default compose file for dev; override with COMPOSE_FILE env var
COMPOSE_FILE_DEFAULT="docker-compose.dev.yml"
COMPOSE_FILE_PATH="${COMPOSE_FILE:-$COMPOSE_FILE_DEFAULT}"

if [[ ! -f "$COMPOSE_FILE_PATH" ]]; then
  echo "❌ No compose file found at '$COMPOSE_FILE_PATH'. Set COMPOSE_FILE or create $COMPOSE_FILE_DEFAULT." >&2
  exit 1
fi

# --- Helpers ---------------------------------------------------------------

dc() {
  # Prefer Docker Compose v2 ("docker compose"), fallback to v1 ("docker-compose")
  if command -v docker >/dev/null && docker compose version >/dev/null 2>&1; then
    docker compose -f "$COMPOSE_FILE_PATH" "$@"
  else
    docker-compose -f "$COMPOSE_FILE_PATH" "$@"
  fi
}

repo_root() { git rev-parse --show-toplevel 2>/dev/null || pwd; }

load_env() {
  # Load .env if present (non-comment, non-empty)
  if [[ -f .env ]]; then
    export $(grep -v '^[[:space:]]*#' .env | grep -v '^[[:space:]]*$' | xargs) || true
  fi
}

wait_for_pg() {
  echo "⏳ Waiting for Postgres to be healthy..."
  for i in {1..30}; do
    if dc ps --services --filter "status=running" | grep -q '^postgres$'; then
      if dc exec -T postgres pg_isready -U "${PG_USER:-pocketpair}" -d "${PG_DB:-pocketpair}" >/dev/null 2>&1; then
        echo "✅ Postgres is ready."
        return 0
      fi
    fi
    sleep 1
  done
  echo "❌ Postgres did not become healthy in time." >&2
  exit 1
}

run_migrations() {
  echo "🚚 Running migrations with sqlx-cli (migrator service)..."
  # Use the dedicated migrator service which mounts ./migrations read-only
  dc run --rm migrator || {
    echo "❌ sqlx migrate run failed." >&2
    exit 1
  }
  echo "✅ Migrations applied."
}

stop_api_if_running() {
  # Stop/remove any existing api container so the port isn't occupied
  if dc ps --services --filter "status=running" | grep -q '^api$'; then
    echo "⏹  Stopping existing api container..."
    dc stop api || true
  fi
  if dc ps --services --filter "status=exited" | grep -q '^api$'; then
    dc rm -f api || true
  fi
}

print_usage() {
  cat <<'USAGE'
Usage: scripts/dev.sh [command]

Commands:
  up         Start Postgres in background, run migrations, then run API in FOREGROUND with cargo watch
  down       Stop containers (keep volumes)
  nuke       Stop containers and remove volumes (DANGER: deletes DB data)
  logs       Tail api logs (when api is running detached)
  migrate    Run sqlx migrations against the running DB (via migrator)
  psql       Open psql shell inside the postgres container
  status     Show compose services status

Tips:
- Configure .env in repo root (PG_DB, PG_USER, PG_PASSWORD, RUST_LOG, etc.)
- Set COMPOSE_FILE to use a different compose file (default: docker-compose.dev.yml)
- API listens on PORT=8080 → http://localhost:8080/health
- Press Ctrl+C to stop the foreground API; Postgres keeps running for fast restarts
USAGE
}

# --- Main ------------------------------------------------------------------

cd "$(repo_root)"

CMD="${1:-up}"

# Force dev target by default; compose will pick Dockerfile.dev via your yml
export DOCKER_ENV="${DOCKER_ENV:-dev}"

echo "Using compose file: $COMPOSE_FILE_PATH"

load_env

case "$CMD" in
  up)
    echo "▶️  Starting dev stack (DOCKER_ENV=$DOCKER_ENV)..."
    # Start Postgres in background
    dc up -d --build postgres
    wait_for_pg
    # Run migrations via dedicated migrator service
    run_migrations
    # Ensure no old API container is holding the port
    stop_api_if_running
    echo "✅ Postgres ready. Launching API in foreground with cargo watch..."
    # Foreground run so you see cargo-watch output. --service-ports publishes 8080
    dc run --rm --service-ports --entrypoint /bin/sh api -lc '
      export PATH="/usr/local/cargo/bin:/usr/local/rustup/bin:$PATH";
      command -v cargo || { echo "cargo not found; PATH=$PATH"; ls -la /usr/local/cargo/bin || true; exit 127; };
      cargo watch -x "run -p api"
    '
    ;;

  down)
    echo "⏹  Stopping containers (keeping volumes)..."
    dc down
    ;;

  nuke)
    echo "💣  Stopping containers and REMOVING VOLUMES (DB DATA WILL BE LOST)..."
    dc down -v
    ;;

  logs)
    dc logs -f api
    ;;

  migrate)
    wait_for_pg
    run_migrations
    ;;

  psql)
    wait_for_pg
    dc exec postgres psql -U "${PG_USER:-pocketpair}" -d "${PG_DB:-pocketpair}"
    ;;

  status)
    dc ps
    ;;

  *)
    print_usage
    exit 1
    ;;
esac