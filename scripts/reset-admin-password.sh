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

: "${TASKTRACKER_ADMIN_EMAIL:=admin@example.com}"

if [ -z "${TASKTRACKER_ADMIN_PASSWORD:=}" ]; then
  echo "ERROR: TASKTRACKER_ADMIN_PASSWORD is not set in .env" >&2
  exit 1
fi

cd "$PROJECT_DIR"

echo "Resetting admin password for ${TASKTRACKER_ADMIN_EMAIL}..."
# Placeholder: replace with actual CLI call once backend exists
docker compose run --rm cli task-tracker users reset-password \
  --email "$TASKTRACKER_ADMIN_EMAIL" \
  --password "$TASKTRACKER_ADMIN_PASSWORD"

echo "Admin password reset."
