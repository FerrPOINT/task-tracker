#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

if [ -z "${TASKTRACKER_DATABASE_URL:-}" ]; then
    echo "Set TASKTRACKER_DATABASE_URL before running E2E tests" >&2
    exit 1
fi
if [ -z "${TASKTRACKER_TEST_DB_PASSWORD:-}" ]; then
    echo "Set TASKTRACKER_TEST_DB_PASSWORD before running E2E tests" >&2
    exit 1
fi

cleanup() {
    docker compose -f "$REPO_ROOT/backend/docker-compose.test.yml" down -v
}
trap cleanup EXIT

docker compose -f "$REPO_ROOT/backend/docker-compose.test.yml" down -v
docker compose -f "$REPO_ROOT/backend/docker-compose.test.yml" up -d

echo "Waiting for test Postgres and Redis to be healthy..."
docker compose -f "$REPO_ROOT/backend/docker-compose.test.yml" exec -T postgres-test sh -c "until pg_isready -U tasktracker -d tasktracker_test; do sleep 1; done"
docker compose -f "$REPO_ROOT/backend/docker-compose.test.yml" exec -T redis-test sh -c "until redis-cli ping | grep -q PONG; do sleep 1; done"

cd backend
cargo llvm-cov --workspace --json --output-path target/llvm-cov/coverage.json -- --include-ignored --test-threads=1

python3 - <<'PY'
import json
with open('target/llvm-cov/coverage.json') as f:
    data = json.load(f)
t=data['data'][0]['totals']
print(f"lines {t['lines']['percent']:.1f}% ({t['lines']['covered']}/{t['lines']['count']})")
print(f"functions {t['functions']['percent']:.1f}% ({t['functions']['covered']}/{t['functions']['count']})")
print(f"regions {t['regions']['percent']:.1f}% ({t['regions']['covered']}/{t['regions']['count']})")
PY
