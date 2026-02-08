# pp-service justfile

set dotenv-load := false

# Test infrastructure
container_name := "pp-test-postgres"
pg_port        := "5433"
pg_user        := "postgres"
pg_pass        := "postgres"
pg_db          := "pocketpair_test"
db_url         := "postgres://" + pg_user + ":" + pg_pass + "@localhost:" + pg_port + "/" + pg_db

# Start a fresh test Postgres container
[private]
test-db-up:
    @docker rm -f {{ container_name }} 2>/dev/null || true
    @docker run -d \
        --name {{ container_name }} \
        -e POSTGRES_USER={{ pg_user }} \
        -e POSTGRES_PASSWORD={{ pg_pass }} \
        -e POSTGRES_DB={{ pg_db }} \
        -p {{ pg_port }}:5432 \
        postgres:16-alpine >/dev/null
    @echo "Waiting for PostgreSQL..."
    @for i in $(seq 1 30); do \
        docker exec {{ container_name }} pg_isready -U {{ pg_user }} -d {{ pg_db }} >/dev/null 2>&1 && break; \
        sleep 1; \
    done

# Tear down test container
[private]
test-db-down:
    @docker rm -f {{ container_name }} 2>/dev/null || true

# Run migrations and seed against test DB
[private]
test-db-seed:
    DATABASE_URL={{ db_url }} cargo sqlx migrate run --source ./migrations
    @PGPASSWORD={{ pg_pass }} psql -h localhost -p {{ pg_port }} -U {{ pg_user }} -d {{ pg_db }} -f scripts/seed.sql -q

# Regenerate .sqlx/ offline cache against test DB
[private]
test-sqlx-prepare:
    DATABASE_URL={{ db_url }} cargo sqlx prepare --workspace -- --tests

# Run all integration tests (spins up fresh DB, runs, tears down)
test *args: test-db-up test-db-seed test-sqlx-prepare
    TEST_DATABASE_URL={{ db_url }} cargo test --package api --tests {{ args }}; \
    status=$?; \
    just test-db-down; \
    exit $status

# Run a single test file (e.g. just test-file clock_lifecycle_tests)
test-file name *args: test-db-up test-db-seed test-sqlx-prepare
    TEST_DATABASE_URL={{ db_url }} cargo test --package api --test {{ name }} {{ args }}; \
    status=$?; \
    just test-db-down; \
    exit $status

# Only regenerate .sqlx/ metadata (needs a running test DB)
sqlx-prepare: test-db-up test-db-seed
    DATABASE_URL={{ db_url }} cargo sqlx prepare --workspace -- --tests
    @just test-db-down
