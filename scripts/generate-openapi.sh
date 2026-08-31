#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OPENAPI_DIR="$ROOT/openapi"
mkdir -p "$OPENAPI_DIR"

cd "$ROOT/backend"
cargo run -p api --bin gen-openapi > "$OPENAPI_DIR/openapi.json"

echo "Saved $OPENAPI_DIR/openapi.json"
