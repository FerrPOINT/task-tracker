# Onboarding — Task Tracker

## 1. Overview

Как развернуть локальную среду новому разработчику за 15 минут.

## 2. Prerequisites

- Docker 24+
- Docker Compose 2.20+
- Rust 1.88+ (`rustup`)
- Node.js 22 LTS + pnpm 10+
- Git + SSH key для GitHub

## 3. Repository

```bash
git clone git@github.com:FerrPOINT/task-tracker.git
cd task-tracker
```

## 4. Environment

```bash
cp .env.example .env
# edit .env (DB password, JWT secrets)
```

## 5. Backend

```bash
cd backend
cargo build
cargo run --bin server
```

## 6. Frontend

```bash
cd frontend
pnpm install
pnpm dev
```

## 7. Run Migrations

```bash
cd backend
cargo run --bin migrator -- up
```

## 8. Seed Data

```bash
cd backend
cargo run --bin seed -- --env development
```

## 9. Run Tests

```bash
# Backend
cargo test

# Frontend
pnpm typecheck
pnpm test
pnpm test:e2e
```

## 10. IDE Setup

- VS Code: extensions rust-analyzer, Tailwind, ESLint, Prettier.
- Format on save.
- Run `cargo clippy` before commit.

## 11. First Contribution

1. Pick issue labeled `good first issue`.
2. Read `docs/AGENTS.md` и `docs/CODE_STYLE.md`.
3. Create branch `feat/short-desc`.
4. Write code + tests.
5. Open PR.

## 12. Common Issues

| Symptom | Fix |
|---------|-----|
| `cargo build` fails | `rustup update` |
| pnpm install fails | `corepack enable && pnpm -v` |
| DB connection refused | `docker compose up -d postgres` |
| WebSocket 403 | Check `TASKTRACKER_CORS_ALLOWED_ORIGINS` |

## References

- `CONTRIBUTING.md` (корень репозитория)
- `docs/DEPLOYMENT.md`
- `docs/TESTING.md`
- `docs/AGENTS.md`
