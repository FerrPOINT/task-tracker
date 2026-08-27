# CI / CD — Task Tracker

## 1. Пайплайн (`.github/workflows/ci.yml`)

Триггеры: push и pull_request в `main`.

| Job | Что делает |
|-----|-----------|
| `backend` | `cargo fmt --check`, `cargo clippy --workspace -D warnings`, `cargo test --workspace` (in-memory стек, без БД) |
| `openapi-check` | `cargo run -p api --bin gen-openapi` → сверка с закоммиченным `openapi/openapi.json` (дрейф = fail) |
| `migrations` | postgres-service в CI; применяет все миграции SeaORM на чистую БД и проверяет статус |
| `coverage` | `cargo-llvm-cov`, порог 60% |
| `infra-db-tests` | postgres-service; docker-backed тесты `infra/tests/repos.rs` + `infra/tests/fk_regression.rs` (`--include-ignored`) |
| `audit` | `cargo audit` (RustSec advisory) |
| `frontend` | `pnpm install`, `pnpm build`, `pnpm test -- --run` (vitest) |
| `e2e` | поднимает compose-стек (postgres, redis, backend), ждёт health, прогоняет `e2e/smoke.spec.ts` (chromium) |

## 2. Локальный эквивалент перед пушем

```bash
cd backend
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cd ../frontend
pnpm typecheck && pnpm lint && pnpm test -- --run && pnpm build
# E2E против живого стека
pnpm exec playwright test --project=chromium
```

## 3. Известные ограничения CI (не покрыто)

- E2E гоняет только `smoke.spec.ts` на chromium — полные спеки и firefox/webkit — локально;
- `cargo deny` (лицензии/дубликаты) не подключён.

(Закрыто в этом цикле: frontend lint/typecheck добавлены в job `frontend`; readiness — retry-цикл до 60с вместо `sleep 20`; docker-backed интеграционные тесты — отдельный job `infra-db-tests`.)

## References

- [TESTING](TESTING.md)
- [LOCAL_SETUP](LOCAL_SETUP.md)
- [OPS_RUNBOOK](OPS_RUNBOOK.md)
