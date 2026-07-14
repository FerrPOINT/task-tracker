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
docker compose exec -T postgres pg_isready -U "${TASKTRACKER_DB_USER:-tasktracker}" -d "${TASKTRACKER_DB_NAME:-tasktracker}" > /dev/null

echo "Running migrations..."
docker compose run --rm migrator

echo "Init complete."
