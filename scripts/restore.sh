#!/usr/bin/env bash
# Restore task-tracker database and file attachments from a backup archive.
#
# Usage:  ./scripts/restore.sh <backup.tar.gz>
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"

if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  set -a; . "$ENV_FILE"; set +a
fi

: "${POSTGRES_USER:=tasktracker}"
: "${POSTGRES_DB:=tasktracker}"

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <backup.tar.gz>" >&2
  exit 1
fi

BACKUP_ARCHIVE="$1"
BACKUP_NAME=$(basename "$BACKUP_ARCHIVE" .tar.gz)
BACKUP_DIR=$(dirname "$BACKUP_ARCHIVE")

if [ ! -f "$BACKUP_ARCHIVE" ]; then
  echo "ERROR: backup archive not found: $BACKUP_ARCHIVE" >&2
  exit 1
fi

cd "$PROJECT_DIR"

echo "Extracting backup..."
tar -xzf "$BACKUP_ARCHIVE" -C "$BACKUP_DIR"

echo "Restoring database..."
docker compose exec -T postgres pg_restore \
  -U "${POSTGRES_USER}" \
  -d "${POSTGRES_DB}" \
  --clean --if-exists \
  < "${BACKUP_DIR}/${BACKUP_NAME}.dump"

echo "Restoring attachments..."
# Resolve the actual volume name via compose (handles project-name prefixes).
UPLOADS_VOLUME=$(docker compose config --volumes uploads 2>/dev/null || echo "task-tracker_uploads")
if [ -f "${BACKUP_DIR}/${BACKUP_NAME}-attachments.tar.gz" ]; then
  docker run --rm \
    -v "${UPLOADS_VOLUME}":/var/lib/tasktracker/uploads \
    -v "${BACKUP_DIR}":/backup:ro \
    --entrypoint /bin/sh \
    debian:bookworm-slim \
    -c "cd /var/lib/tasktracker/uploads && tar -xzf \"/backup/${BACKUP_NAME}-attachments.tar.gz\" && chown -R 999:999 /var/lib/tasktracker/uploads"
else
  echo "WARNING: no attachments archive found in backup; skipping" >&2
fi

echo "Restore complete."