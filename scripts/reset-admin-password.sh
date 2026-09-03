#!/usr/bin/env bash
# Reset an admin user's password directly in the database.
#
# This bypasses the API (useful when the admin cannot log in) by setting a
# new Argon2id hash directly.  Generate the hash with the backend CLI or
# any argon2 tool.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"

if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  set -a; . "$ENV_FILE"; set +a
fi

: "${TASKTRACKER_ADMIN_EMAIL:=admin@example.com}"

if [ -z "${TASKTRACKER_ADMIN_PASSWORD:=}" ]; then
  echo "ERROR: TASKTRACKER_ADMIN_PASSWORD is not set in .env" >&2
  exit 1
fi

cd "$PROJECT_DIR"

# Use the backend container to hash the password via argon2, then update DB.
echo "Hashing new password and updating ${TASKTRACKER_ADMIN_EMAIL}..."
docker compose exec -T backend task-tracker-server --hash-password "${TASKTRACKER_ADMIN_PASSWORD}" 2>/dev/null \
  | xargs -I{} docker compose exec -T postgres \
    psql -U "${POSTGRES_USER:-tasktracker}" -d "${POSTGRES_DB:-tasktracker}" \
    -v ON_ERROR_STOP=1 \
    -c "UPDATE users SET password_hash = '{}' WHERE email = '${TASKTRACKER_ADMIN_EMAIL}';" \
  || {
    echo "FALLBACK: Use the CLI to reset the password after logging in:" >&2
    echo "  docker compose exec backend task-tracker admin create-user --email ${TASKTRACKER_ADMIN_EMAIL} ..." >&2
    exit 1
  }

echo "Admin password reset."