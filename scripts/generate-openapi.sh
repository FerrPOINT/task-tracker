#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OPENAPI_DIR="$ROOT/openapi"
mkdir -p "$OPENAPI_DIR"

cd "$ROOT/backend"
cargo build --bin openapi-gen
./target/debug/openapi-gen "$OPENAPI_DIR/openapi.json"

echo "Saved $OPENAPI_DIR/openapi.json"
