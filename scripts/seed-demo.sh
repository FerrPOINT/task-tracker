#!/usr/bin/env bash
# Seed demo data via the task-tracker CLI.
#
# Requires a running backend and an existing admin token.
# Set TASKTRACKER_TOKEN and TASKTRACKER_API_URL env vars (or .env).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"

if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  set -a; . "$ENV_FILE"; set +a
fi

cd "$PROJECT_DIR"

if [ -z "${TASKTRACKER_TOKEN:-}" ]; then
  echo "ERROR: TASKTRACKER_TOKEN is not set. Log in first:" >&2
  echo "  docker compose exec backend task-tracker auth login --email admin@example.com --password ..." >&2
  exit 1
fi

echo "Seeding demo data..."
# Create a demo project and issues via the CLI.
docker compose exec -T backend task-tracker \
  --api-url "${TASKTRACKER_API_URL:-http://localhost:3456/api/v1}" \
  --token "${TASKTRACKER_TOKEN}" \
  project create --name "Demo" --key "DEMO" --description "Demo project" || true

docker compose exec -T backend task-tracker \
  --api-url "${TASKTRACKER_API_URL:-http://localhost:3456/api/v1}" \
  --token "${TASKTRACKER_TOKEN}" \
  issue create --project DEMO --summary "Sample task 1" || true

docker compose exec -T backend task-tracker \
  --api-url "${TASKTRACKER_API_URL:-http://localhost:3456/api/v1}" \
  --token "${TASKTRACKER_TOKEN}" \
  issue create --project DEMO --summary "Sample task 2" || true

echo "Demo data seeded."