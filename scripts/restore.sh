#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"

if [ -f "$ENV_FILE" ]; then
  # shellcheck source=/dev/null
  set -a
  # shellcheck source=/dev/null
  . "$ENV_FILE"
  set +a
fi

: "${TASKTRACKER_DB_USER:=tasktracker}"
: "${TASKTRACKER_DB_NAME:=tasktracker}"

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
  -U "$TASKTRACKER_DB_USER" \
  -d "$TASKTRACKER_DB_NAME" \
  --clean --if-exists \
  < "${BACKUP_DIR}/${BACKUP_NAME}.dump"

echo "Restoring attachments..."
if [ -d "${BACKUP_DIR}/${BACKUP_NAME}-attachments" ]; then
  mkdir -p data/attachments
  cp -a "${BACKUP_DIR}/${BACKUP_NAME}-attachments/." data/attachments/
fi

echo "Restore complete."
