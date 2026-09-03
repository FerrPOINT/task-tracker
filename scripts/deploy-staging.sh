#!/usr/bin/env bash
# Deploy task-tracker to a staging environment.
#
# Staging uses the same docker-compose.yml as production but may override
# ports and other settings via a .env file.  Migrations run automatically at
# backend startup, so no separate migrator service is needed.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

git pull origin main
docker compose build
docker compose up -d

echo "Staging deployed."