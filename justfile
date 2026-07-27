# Task Tracker — unified dev commands
# Requires: just, cargo, pnpm, docker compose

set fallback
set shell := ["bash", "-uc"]

# Default recipe: show available commands
default:
    @just --list

# ─────────────────────────────────────────────
# Setup
# ─────────────────────────────────────────────

# Install all dependencies (backend + frontend)
setup:
    cd backend && cargo fetch
    cd frontend && pnpm install

# Copy env example if .env is missing
setup-env:
    @if [ ! -f .env ]; then cp .env.example .env && echo "Created .env from .env.example"; else echo ".env already exists"; fi

# ─────────────────────────────────────────────
# Development
# ─────────────────────────────────────────────

# Start Docker infrastructure (Postgres + backend)
db-up:
    docker compose up -d

# Stop Docker infrastructure
db-down:
    docker compose down

# Run backend dev server via Docker with live reload
backend-dev:
    docker compose up -d
    @echo "Backend at http://127.0.0.1:3456"

# Run frontend dev server
frontend-dev:
    cd frontend && pnpm dev

# Run backend + frontend in separate tmux windows (if tmux available)
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v tmux >/dev/null 2>&1; then
      tmux new-session -d -s task-tracker "just backend-dev"
      tmux split-window -h -t task-tracker "just frontend-dev"
      tmux attach -t task-tracker
    else
      echo "tmux not found. Run in two terminals: just backend-dev | just frontend-dev"
    fi

# ─────────────────────────────────────────────
# Quality gates
# ─────────────────────────────────────────────

# Format Rust code
fmt-rust:
    cd backend && cargo fmt --all

# Format frontend code
fmt-frontend:
    cd frontend && pnpm format

# Format all
fmt: fmt-rust fmt-frontend

# Check Rust formatting
fmt-check-rust:
    cd backend && cargo fmt --all -- --check

# Lint Rust
clippy:
    cd backend && cargo clippy --workspace --all-targets

# Typecheck frontend
typecheck:
    cd frontend && pnpm typecheck

# Run frontend unit tests
test-frontend:
    cd frontend && pnpm test

# Run backend unit + integration tests (single-threaded for env safety)
test-backend:
    cd backend && cargo test --workspace -- --test-threads=1

# Run backend coverage gate (requires DB password in TT_DB_PASS)
test-backend-coverage:
    cd backend && bash scripts/run-e2e-tests.sh

# Run all fast tests
@test: test-backend test-frontend

# Run Playwright E2E tests against running backend
e2e:
    cd frontend && pnpm exec playwright test

# Full quality gate (CI-like)
gate: fmt-check-rust clippy typecheck test-frontend test-backend

# ─────────────────────────────────────────────
# Production
# ─────────────────────────────────────────────

# Build production frontend
build-frontend:
    cd frontend && pnpm build

# Build backend release binary
build-backend:
    cd backend && cargo build --release

# Build everything
build: build-backend build-frontend

# ─────────────────────────────────────────────
# Misc
# ─────────────────────────────────────────────

# Regenerate OpenAPI client from backend spec
api-codegen:
    cd frontend && pnpm generate-api

# Open backend API docs (requires running backend)
api-docs:
    xdg-open http://127.0.0.1:3456/api-docs 2>/dev/null || open http://127.0.0.1:3456/api-docs

# Check git status
git-status:
    git status --short

# Clean build artifacts
clean:
    cd backend && cargo clean
    cd frontend && rm -rf dist node_modules/.vitest
