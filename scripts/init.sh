#!/usr/bin/env bash
# Initialize a fresh task-tracker installation.
#
# Creates .env from .env.example if missing, starts Postgres + Redis,
# and waits for health.  Migrations are applied automatically when the
# backend container starts (server/src/lib.rs).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="${PROJECT_DIR}/.env"

cd "$PROJECT_DIR"

if [ ! -f docker-compose.yml ]; then
  echo "ERROR: docker-compose.yml not found in $PROJECT_DIR" >&2
  exit 1
fi

mkdir -p traefik/letsencrypt backups

if [ ! -f .env ]; then
  echo "Creating .env from .env.example..."
  cp .env.example .env
  echo "Please edit .env before next run."
  exit 0
fi

echo "Starting infrastructure..."
docker compose up -d postgres redis

echo "Waiting for postgres healthy..."
# shellcheck disable=SC1090
if [ -f "$ENV_FILE" ]; then
  set -a; . "$ENV_FILE"; set +a
fi
PG_USER="${POSTGRES_USER:-tasktracker}"
PG_DB="${POSTGRES_DB:-tasktracker}"
docker compose exec -T postgres pg_isready -U "${PG_USER}" -d "${PG_DB}" > /dev/null

echo "Starting backend (migrations run on startup)..."
docker compose up -d backend

echo "Init complete."