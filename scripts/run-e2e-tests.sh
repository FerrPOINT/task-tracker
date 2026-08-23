#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

if [ -z "${TASKTRACKER_DATABASE_URL:-}" ]; then
    echo "Set TASKTRACKER_DATABASE_URL before running E2E tests" >&2
    exit 1
fi
# Not required when the test Postgres uses trust auth (docker-compose.test.yml default).

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
import json, sys

# Stable-Rust realistic gate. `functions` is structurally capped below 100% by
# compiler-generated async closures and duplicate monomorphization across test
# binaries; migrations `down` and entry-point binaries are excluded upstream.
THRESHOLDS = {"lines": 77.0, "regions": 70.0, "functions": 63.0}

with open('target/llvm-cov/coverage.json') as f:
    data = json.load(f)
t = data['data'][0]['totals']
failed = False
for metric in ("lines", "regions", "functions"):
    pct = t[metric]['percent']
    ok = pct >= THRESHOLDS[metric]
    print(f"{metric:9} {pct:6.2f}% (gate >= {THRESHOLDS[metric]:.1f}%) {'OK' if ok else 'FAIL'}")
    if not ok:
        failed = True
sys.exit(1 if failed else 0)
PY
