#!/usr/bin/env bash
#
# PostgreSQL backup for PocketPair. Dumps the database, gzips it, writes it to
# BACKUP_DIR, and prunes dumps older than RETENTION_DAYS.
#
# Schedule it (host cron, every 6h):
#   0 */6 * * *  DATABASE_URL=... BACKUP_DIR=/var/backups/pocketpair \
#                /path/to/pp-service/deploy/backup.sh >> /var/log/pp-backup.log 2>&1
#
# Or run it from the db-backup service in docker-compose.prod.example.yml.
#
# IMPORTANT: BACKUP_DIR should live on storage that is OFF the database host (a
# mounted volume synced to object storage, an NFS share, etc.). A backup on the
# same disk does not survive that disk failing. Test a restore periodically:
#   gunzip -c <dump>.sql.gz | psql "$DATABASE_URL"
set -euo pipefail

: "${DATABASE_URL:?set DATABASE_URL (postgres://user:pass@host:port/db)}"
BACKUP_DIR="${BACKUP_DIR:-/var/backups/pocketpair}"
RETENTION_DAYS="${RETENTION_DAYS:-14}"

mkdir -p "$BACKUP_DIR"

# Timestamp is intentionally read at runtime (the cron invocation), not baked in.
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="$BACKUP_DIR/pocketpair_${stamp}.sql.gz"

echo "[$(date -u)] dumping database -> $out"
pg_dump --no-owner --no-privileges "$DATABASE_URL" | gzip -9 > "$out"

echo "[$(date -u)] pruning dumps older than ${RETENTION_DAYS} days"
find "$BACKUP_DIR" -name 'pocketpair_*.sql.gz' -type f -mtime "+${RETENTION_DAYS}" -delete

echo "[$(date -u)] backup complete ($(du -h "$out" | cut -f1))"
