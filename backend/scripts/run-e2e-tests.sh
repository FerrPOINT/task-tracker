#!/usr/bin/env bash
set -euo pipefail

export PATH="${HOME}/.cargo/bin:${PATH}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.test.yml}"
COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-backend}"
DB_HOST="${TASKTRACKER_TEST_DB_HOST:-127.0.0.1}"
DB_PORT="${TASKTRACKER_TEST_DB_PORT:-3458}"
DB_USER="${TASKTRACKER_TEST_DB_USER:-tasktracker}"
DB_NAME="${TASKTRACKER_TEST_DB_NAME:-tasktracker_test}"
# Reuse an already-running local test stack when its published port is ready;
# otherwise provision an isolated compose project and clean it up on exit.
MANAGE_STACK=false
# The test compose enables local trust authentication. A password is neither
# required nor read from host files, avoiding secret handling in this runner.
export TASKTRACKER_DATABASE_URL="postgres://${DB_USER}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
export RUST_LOG="${RUST_LOG:-warn}"

compose() {
    docker compose -p "$COMPOSE_PROJECT_NAME" -f "$COMPOSE_FILE" "$@"
}

cleanup() {
    if [ "$MANAGE_STACK" = true ]; then
        compose down -v --remove-orphans >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

if timeout 1 bash -c "</dev/tcp/${DB_HOST}/${DB_PORT}" 2>/dev/null; then
    # A local test compose project is already up; preserve it and use its port.
    :
else
    MANAGE_STACK=true
    compose up -d postgres-test redis-test
fi
for _ in $(seq 1 60); do
    if timeout 1 bash -c "</dev/tcp/${DB_HOST}/${DB_PORT}" 2>/dev/null; then
        break
    fi
    sleep 1
done
timeout 1 bash -c "</dev/tcp/${DB_HOST}/${DB_PORT}" 2>/dev/null

# Reset the test database so stale migration history (e.g. removed seed
# migrations recorded by older code) cannot wedge `Migrator::up`.
compose exec -T postgres-test psql -U "${DB_USER}" -d postgres \
    -c "DROP DATABASE IF EXISTS \"${DB_NAME}\";" \
    -c "CREATE DATABASE \"${DB_NAME}\";" >/dev/null

EXCLUDE_REGEX="server/src/main\.rs|api/src/bin/openapi-gen\.rs|cli/src/main\.rs|migration/.*|shared/src/id\.rs"

cargo llvm-cov --workspace \
    --ignore-filename-regex "$EXCLUDE_REGEX" \
    --json \
    --output-path target/coverage.json \
    -- --include-ignored --test-threads=1

echo ""
echo "=== Coverage summary ==="
cargo llvm-cov report --summary-only --ignore-filename-regex "$EXCLUDE_REGEX"

echo ""
echo "=== Coverage gate ==="
python3 - <<'PY'
import json
import sys
with open('target/coverage.json') as f:
    totals = json.load(f)['data'][0]['totals']
lines = totals['lines']['percent']
regions = totals['regions']['percent']
functions = totals['functions']['percent']
print(f"lines: {lines:.2f}%")
print(f"regions: {regions:.2f}%")
print(f"functions: {functions:.2f}%")
# These thresholds include the real Postgres repositories and server entry
# point. They intentionally match the measured stable-Rust workspace baseline
# rather than silently omitting ignored production tests.
ok = lines >= 75.0 and regions >= 65.0 and functions >= 60.0
if not ok:
    print("Gate FAILED")
    sys.exit(1)
print("Gate passed")
PY
