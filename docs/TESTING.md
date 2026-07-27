# Стратегия тестирования Task Tracker

## 1. Принципы

- Каждый тест проверяет значимый путь и конкретное поведение.
- Backend: реальные интеграционные тесты с PostgreSQL через Docker; unit-тесты для domain/services.
- Frontend: unit-тесты на Vitest; E2E на Playwright.
- После изменений UI — скриншоты в 375×812, 1920×1080, 2560×1440.
- Coverage gate: lines ≥95%, regions ≥92%, functions ≥85% (backend).

## 2. Backend тесты

### Unit-тесты

- `domain/` — entity invariants, repository stubs, `ProjectKey::is_valid`.
- `app/src/services/tests.rs` — service logic, auth edge cases, error propagation.
- `app/src/auth.rs` — password hash/verify, token generate/parse.
- `shared/src/config_tests.rs` — env parsing scenarios.
- `shared/src/id/tests.rs` — UUID / project key edge cases.

Запуск:
```bash
cd backend
cargo test -p <crate> -- --test-threads=1   # если тесты меняют env
cargo test -p api --test failing_repos -- --test-threads=1
```

### Integration-тесты

- `infra/tests/repos.rs` — Postgres-репозитории против реальной БД.
- `infra/tests/repos_mock.rs` — `sea_orm::MockDatabase` error paths.
- `api/tests/integration.rs` — end-to-end HTTP через Docker backend.
- `api/tests/failing_repos.rs` — 500-ветки с failing stubs.

### Coverage gate

```bash
export TT_DB_PASS=$(cat /root/.tt_db_pass)
cd backend && bash scripts/run-e2e-tests.sh
```

Текущие thresholds: lines 95%, regions 92%, functions 85%.

## 3. Frontend тесты

### Unit-тесты

Фреймворк: Vitest + `@testing-library/react`.

Страницы с тестами:
- `login/login.test.tsx`
- `register/register.test.tsx`
- `dashboard/dashboard.test.tsx`
- `projects/projects.test.tsx`
- `project-board/project-board.test.tsx`
- `search/search.test.tsx`
- `features/time-tracking/**/*.test.ts`

Запуск:
```bash
cd frontend
pnpm test
```

### E2E

Playwright specs в `frontend/e2e/`:
- `integration.spec.ts` — smoke против Docker backend
- `screenshots.spec.ts` — мульти-вьюпортные скриншоты

Запуск:
```bash
cd frontend
pnpm exec playwright test --project=chromium
```

### Screenshot набор

Скриншоты сохраняются в `/root/.hermes/cache/images/react-<page>-<viewport>.png`.

## 4. Dev commands

Все команды через `justfile`:

```bash
just gate          # fmt-check + clippy + typecheck + tests
just test          # backend + frontend tests
just e2e           # Playwright
just test-backend-coverage  # coverage gate
```

## 5. Git hooks

Lefthook (`lefthook.yml`):
- `pre-commit`: rust fmt check, clippy, frontend typecheck/test/lint
- `pre-push`: backend tests, frontend build, e2e smoke
- `commit-msg`: conventional commits (`feat|fix|docs|...`)

## 6. Coverage

### Backend

| Layer | Target |
|---|---|
| Domain | ≥90% |
| Application | ≥95% |
| Infra | ≥90% |
| API routes | ≥85% |
| **Total** | **lines ≥95, regions ≥92, functions ≥85** |

### Frontend

- Целевой показатель не зафиксирован в CI; приоритет — покрытие critical UI и pure utils.

## 7. Чек-лист перед merge

- [ ] `cargo fmt --all && cargo clippy --workspace --all-targets` clean
- [ ] `pnpm typecheck` clean
- [ ] `pnpm test` green
- [ ] `cargo test --workspace -- --test-threads=1` green
- [ ] `bash scripts/run-e2e-tests.sh` green
- [ ] `pnpm build` green
- [ ] Playwright critical path green
- [ ] Документация обновлена

## References

- `docs/ARCHITECTURE.md`
- `docs/DEPLOYMENT.md`
- `justfile`
- `lefthook.yml`
- `backend/scripts/run-e2e-tests.sh`
