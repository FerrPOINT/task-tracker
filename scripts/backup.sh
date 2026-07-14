#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"
BACKUP_DIR="${PROJECT_DIR}/backups"

if [ -f "$ENV_FILE" ]; then
  # shellcheck source=/dev/null
  set -a
  # shellcheck source=/dev/null
  . "$ENV_FILE"
  set +a
fi

: "${TASKTRACKER_DB_USER:=tasktracker}"
: "${TASKTRACKER_DB_NAME:=tasktracker}"

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_NAME="task-tracker-${TIMESTAMP}"
BACKUP_PATH="${BACKUP_DIR}/${BACKUP_NAME}"

mkdir -p "$BACKUP_DIR"
cd "$PROJECT_DIR"

echo "Backing up database..."
docker compose exec -T postgres pg_dump \
  -U "$TASKTRACKER_DB_USER" \
  -d "$TASKTRACKER_DB_NAME" \
  -Fc \
  > "${BACKUP_PATH}.dump"

echo "Backing up attachments..."
if [ -d "data/attachments" ]; then
  cp -a "data/attachments" "${BACKUP_PATH}-attachments"
fi

echo "Creating archive..."
tar -czf "${BACKUP_PATH}.tar.gz" -C "$BACKUP_DIR" \
  "${BACKUP_NAME}.dump" \
  "${BACKUP_NAME}-attachments" 2>/dev/null || true

rm -rf "${BACKUP_PATH}.dump" "${BACKUP_PATH}-attachments"

echo "Backup created: ${BACKUP_PATH}.tar.gz"
