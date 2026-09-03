#!/usr/bin/env bash
# Deploy task-tracker to a production-like environment.
#
# This script pulls latest main, rebuilds and restarts the Docker Compose
# stack.  Migrations run automatically at backend startup (server/src/lib.rs),
# so no separate migrator service is needed.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

git pull origin main
docker compose build
docker compose up -d

echo "Production deployed."