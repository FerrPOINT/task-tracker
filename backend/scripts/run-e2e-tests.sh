#!/usr/bin/env bash
set -euo pipefail

export PATH="${HOME}/.cargo/bin:${PATH}"

cd "$(dirname "$0")/.."

DB_HOST="127.0.0.1"
DB_PORT="3457"
DB_USER="tasktracker_test"
DB_NAME="tasktracker_test"
PASS_FILE="${PASS_FILE:-/root/.tt_db_pass}"
if [ ! -f "$PASS_FILE" ]; then
    echo "Password file not found: $PASS_FILE" >&2
    exit 1
fi
TT_DB_PASS="$(cat "$PASS_FILE")"
export TASKTRACKER_DATABASE_URL="postgres://${DB_USER}:${TT_DB_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
export RUST_LOG="warn"

psql_exec() {
    docker exec task-tracker-postgres-1 psql -U "${DB_USER}" -d postgres "$@"
}

if ! psql_exec -tc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'" | grep -q 1; then
    docker exec task-tracker-postgres-1 createdb -U "${DB_USER}" "${DB_NAME}"
fi

EXCLUDE_REGEX="server/src/main\.rs|api/src/bin/openapi-gen\.rs|cli/src/main\.rs|migration/.*|shared/src/id\.rs"

cargo llvm-cov --workspace \
    --ignore-filename-regex "${EXCLUDE_REGEX}" \
    --json \
    --output-path target/coverage.json \
    -- --include-ignored --test-threads=1

echo ""
echo "=== Coverage summary ==="
cargo llvm-cov report --summary-only --ignore-filename-regex "${EXCLUDE_REGEX}"

echo ""
echo "=== Coverage gate ==="
python3 - <<'PY'
import json, sys
with open('target/coverage.json') as f:
    data = json.load(f)
totals = data['data'][0]['totals']
lines = totals['lines']['percent']
regions = totals['regions']['percent']
functions = totals['functions']['percent']
print(f"lines: {lines:.2f}%")
print(f"regions: {regions:.2f}%")
print(f"functions: {functions:.2f}%")
ok = lines >= 91.0 and regions >= 86.0 and functions >= 78.0
sys.exit(0 if ok else 1)
PY
echo "Gate passed"
