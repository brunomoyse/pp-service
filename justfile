# pp-service justfile

set dotenv-load := false

# Read DATABASE_URL from .env for dev commands
dev-db-url := `grep '^DATABASE_URL=' .env | sed 's/^DATABASE_URL=//' | tr -d '"' | tr -d "'"`

# --- Test recipes (testcontainers handles DB automatically) ---

# Stop and remove any leftover testcontainers postgres instances
[private]
cleanup-testcontainers:
    #!/usr/bin/env bash
    ids=$(docker ps -aq --filter "label=org.testcontainers.managed-by=testcontainers")
    if [ -n "$ids" ]; then
        echo "Cleaning up testcontainers..."
        docker rm -f $ids >/dev/null
    fi

# Run all integration tests
test *args:
    SQLX_OFFLINE=true cargo test --package api --tests {{ args }}; \
    status=$?; \
    just cleanup-testcontainers; \
    exit $status

# Run a single test file (e.g. just test-file clock_lifecycle_tests)
test-file name *args:
    SQLX_OFFLINE=true cargo test --package api --test {{ name }} {{ args }}; \
    status=$?; \
    just cleanup-testcontainers; \
    exit $status

# Regenerate .sqlx/ offline cache (needs ephemeral container for DATABASE_URL)
sqlx-prepare:
    #!/usr/bin/env bash
    set -euo pipefail
    CONTAINER=$(docker run -d --rm \
        -e POSTGRES_USER=postgres \
        -e POSTGRES_PASSWORD=postgres \
        -e POSTGRES_DB=pocketpair \
        -p 0:5432 \
        postgres:16-alpine)
    # Find the mapped port
    PORT=$(docker port "$CONTAINER" 5432 | head -1 | cut -d: -f2)
    echo "Waiting for PostgreSQL on port $PORT..."
    for i in $(seq 1 30); do
        docker exec "$CONTAINER" pg_isready -U postgres -d pocketpair >/dev/null 2>&1 && break
        sleep 1
    done
    DB_URL="postgres://postgres:postgres@localhost:${PORT}/pocketpair"
    DATABASE_URL="$DB_URL" cargo sqlx migrate run --source ./migrations
    DATABASE_URL="$DB_URL" cargo sqlx prepare --workspace -- --tests
    docker rm -f "$CONTAINER" >/dev/null

# --- Dev database recipes (operate on .env DATABASE_URL) ---

# Run migrations against dev database
migrate:
    DATABASE_URL={{ dev-db-url }} cargo sqlx migrate run --source ./migrations

# Seed dev database with all fixtures in order
seed:
    #!/usr/bin/env bash
    set -euo pipefail
    for f in fixtures/*.sql; do
        echo "Running $f..."
        psql "{{ dev-db-url }}" -f "$f" -q
    done

# Reset dev database (drop + create + migrate + seed)
db-reset:
    #!/usr/bin/env bash
    set -euo pipefail
    DATABASE_URL="{{ dev-db-url }}" cargo sqlx database reset -y --source ./migrations
    for f in fixtures/*.sql; do
        echo "Running $f..."
        psql "{{ dev-db-url }}" -f "$f" -q
    done
