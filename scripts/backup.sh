#!/usr/bin/env bash
# Backup task-tracker database and file attachments.
#
# Usage:  ./scripts/backup.sh [output-dir]
#
# If no output-dir is given, defaults to ${PROJECT_DIR}/backups.
# Reads POSTGRES_USER / POSTGRES_DB from .env (same names as docker-compose.yml).
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

BACKUP_DIR="${1:-${PROJECT_DIR}/backups}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_NAME="task-tracker-${TIMESTAMP}"
BACKUP_PATH="${BACKUP_DIR}/${BACKUP_NAME}"

mkdir -p "$BACKUP_DIR"
cd "$PROJECT_DIR"

echo "Backing up database..."
docker compose exec -T postgres pg_dump \
  -U "${POSTGRES_USER}" \
  -d "${POSTGRES_DB}" \
  -Fc \
  > "${BACKUP_PATH}.dump"

echo "Backing up attachments..."
# Resolve the actual volume name via compose (handles project-name prefixes).
UPLOADS_VOLUME=$(docker compose config --volumes uploads 2>/dev/null || echo "task-tracker_uploads")
docker run --rm \
  -v "${UPLOADS_VOLUME}":/var/lib/tasktracker/uploads:ro \
  -v "${BACKUP_DIR}":/backup \
  --entrypoint /bin/tar \
  debian:bookworm-slim \
  -czf "/backup/${BACKUP_NAME}-attachments.tar.gz" \
  -C /var/lib/tasktracker/uploads .

echo "Creating archive..."
tar -czf "${BACKUP_PATH}.tar.gz" -C "$BACKUP_DIR" \
  "${BACKUP_NAME}.dump" \
  "${BACKUP_NAME}-attachments.tar.gz"

rm -f "${BACKUP_PATH}.dump" "${BACKUP_PATH}-attachments.tar.gz"

echo "Backup created: ${BACKUP_PATH}.tar.gz"