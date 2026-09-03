#!/usr/bin/env bash
# Create an admin user via the CLI running inside the backend container.
#
# The backend container already has the task-tracker binary; no separate
# compose service is needed.  The CLI admin create-user command calls
# POST /api/v1/admin/users which requires an existing system admin.
#
# For the very first admin user, seed it directly via SQL or register through
# the API and then promote the user to system_admin in the database:
#   docker compose exec postgres psql -U tasktracker -d tasktracker \
#     -c "UPDATE users SET is_system_admin = true WHERE email = 'admin@example.com'"
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"

if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  set -a; . "$ENV_FILE"; set +a
fi

: "${TASKTRACKER_ADMIN_EMAIL:=admin@example.com}"
: "${TASKTRACKER_ADMIN_PASSWORD:=}"

cd "$PROJECT_DIR"

if [ -z "$TASKTRACKER_ADMIN_PASSWORD" ]; then
  echo "ERROR: TASKTRACKER_ADMIN_PASSWORD is not set in .env" >&2
  echo "Hint: also ensure TASKTRACKER_JWT_SECRET and POSTGRES_PASSWORD are set." >&2
  exit 1
fi

# Promote the user to system admin directly in the database — this works
# even for the very first admin when none exists yet.
echo "Promoting ${TASKTRACKER_ADMIN_EMAIL} to system admin..."
docker compose exec -T postgres \
  psql -U "${POSTGRES_USER:-tasktracker}" -d "${POSTGRES_DB:-tasktracker}" \
  -v ON_ERROR_STOP=1 \
  -c "UPDATE users SET is_system_admin = true, is_active = true WHERE email = '${TASKTRACKER_ADMIN_EMAIL}';"

echo "Admin promoted."