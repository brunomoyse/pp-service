# Deploy templates

Production hardening artifacts for the audit's Tier-2 ops items. They live here
(a tracked repo) because the orchestrating `docker-compose.yml` is at the
monorepo root, which is **not** under version control. Adapt and apply them on
the deployment host.

## Files

| File | Purpose |
|------|---------|
| `docker-compose.prod.example.yml` | Postgres + backend + `db-backup`, with resource limits and production env (`ALLOWED_ORIGINS`, `GQL_INTROSPECTION=false`). |
| `backup.sh` | `pg_dump` → gzip → rotate. Run from the `db-backup` service or host cron. |
| `.env.production.example` | Env template; copy to `.env.production` and fill from your secret manager. |

## Use

```bash
cd pp-service/deploy
cp .env.production.example .env.production   # then fill in secrets
docker compose -f docker-compose.prod.example.yml --env-file .env.production up -d
```

## Tune before production

- **Resource limits** in the compose file are starting points — size `cpus`/`memory`
  to your host and expected tournament concurrency, then watch real usage.
- **Backups must go off-host.** `./backups` on the DB host does not survive that
  host failing — sync it to object storage (Scaleway/S3) or a separate volume,
  and **test a restore**: `gunzip -c <dump>.sql.gz | psql "$DATABASE_URL"`.
- **Secrets** come from your secret manager, never the repo. Rotate `JWT_SECRET`
  and DB credentials on a schedule.
- Behind a reverse proxy (Caddy), make sure it sets `X-Forwarded-For` — the
  backend's `/graphql` rate limiter keys on it.

## Still owner actions (not code)

- Hosted monitoring + alerting (the backend exposes `/health`; Tier 3 adds
  metrics/readiness).
- Where backups are shipped and how restores are drilled.
- Standing up a separate **staging** backend so `pp-player-app`'s preview channel
  stops sharing production (the `staging` EAS profile is ready for it).
