#!/usr/bin/env bash
set -euo pipefail

# cd to repo root (script can be called from anywhere)
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

# Load .env if present (PG_* or DATABASE_URL)
if [[ -f .env ]]; then
  # shellcheck disable=SC2046
  export $(grep -v '^#' .env | xargs) || true
fi

# Build a default DATABASE_URL if not provided
if [[ -z "${DATABASE_URL:-}" ]]; then
  PG_DB="${PG_DB:-pocketpair}"
  PG_USER="${PG_USER:-pocketpair}"
  PG_PASSWORD="${PG_PASSWORD:-pocketpair}"
  # Default to the docker-compose Postgres published on localhost:6432
  export DATABASE_URL="postgres://${PG_USER}:${PG_PASSWORD}@127.0.0.1:6432/${PG_DB}"
fi

echo "Using DATABASE_URL=${DATABASE_URL}"

# Pick sqlx-cli: prefer local binary, fallback to a containerized CLI
if command -v sqlx >/dev/null 2>&1; then
  echo "Running migrations with local sqlx-cli..."
  sqlx migrate run --source migrations
else
  echo "Local sqlx-cli not found. Using containerized sqlx-cli..."
  # Adjust version if you want to pin it
  SQLX_VER="${SQLX_VER:-0.8.1}"

  docker run --rm \
    -e DATABASE_URL="${DATABASE_URL}" \
    -v "$(pwd)/migrations:/migrations:ro" \
    ghcr.io/launchbadge/sqlx-cli:${SQLX_VER} \
    migrate run --source /migrations
fi

echo "✅ Migrations applied."