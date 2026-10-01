# Стратегия тестирования Task Tracker

## Tracker SDLC Verification

`cargo test -p app --lib sdlc::tests` checks answer modes/custom text, stable
option validation, stale fences, machine/human separation, canonical payload
hashes, JavaScript integer bounds and exact revision readiness.

With an isolated disposable PostgreSQL database:

```bash
TT_SDLC_TEST_DATABASE_URL=postgres://user:password@host/tasktracker_sdlc_test \
  cargo test -p server --test sdlc -- --ignored --nocapture
```

The test migrates and truncates only that explicit database. It uses a generated
test ES256 issuer/live-session endpoint and PAT introspection fixture, and real
Tracker HTTP plus PostgreSQL. It covers strict membership despite configured
Central Auth bypass, wrong audience/expiry/local credentials, machine scopes,
cross-project root rejection, owner-only writes, concurrency, outbox rollback,
exact confirmation, immutable instance and service restart readback.
It is Tracker integration evidence, not a real Central Auth/Fleet/Workflow/PM
deployment acceptance. Exact performed gates and limitations are recorded in
[CHAT_CLARIFICATION_VERIFICATION.md](CHAT_CLARIFICATION_VERIFICATION.md).

## 1. Принципы

- Каждый тест проверяет значимый путь и конкретное поведение.
- Backend: реальные интеграционные тесты с PostgreSQL через Docker; unit-тесты для domain/services.
- Frontend: unit-тесты на Vitest; E2E на Playwright.
- После изменений UI — скриншоты в 375×812, 1920×1080, 2560×1440.
- Coverage gate в CI: ≥60% (`cargo-llvm-cov`, job `coverage`); локальный полный прогон — `just test-backend-coverage`.

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

- `api/tests/integration.rs` — end-to-end HTTP на in-memory стеке (spawn axum + memory-репозитории); ~130 сценариев.
- `api/tests/failing_repos.rs` — 500-ветки с failing stubs.
- `api/tests/middleware.rs` — JWT-middleware.
- `infra/tests/repos_mock.rs` — `sea_orm::MockDatabase` error paths.

### Docker-backed тесты (Postgres; `--include-ignored`)

- `infra/tests/repos.rs` — Postgres-репозитории против реальной БД (`tasktracker_infra_test`).
- `infra/tests/fk_regression.rs` — FK-констрейнты миграции m20260827_0000028 (orphan-вставки отклоняются, все констрейнты validated).

```bash
# подготовить тест-БД и запустить
cd backend && cargo test -p infra --test repos --test fk_regression -- --include-ignored --test-threads=1
```

### Coverage gate

```bash
# Требуются Rust 1.88, cargo-llvm-cov и Docker Compose; disposable test stack
# использует local trust authentication и по умолчанию serial Cargo build, чтобы
# не исчерпать память runner-а. Пароль из host files не читается.
cd backend && ./scripts/run-e2e-tests.sh
```

CI-порог покрытия — 60% (`coverage` job); цель по слоям ниже — ориентир, не гейт.

## 3. Frontend тесты

Live Playwright использует ожидания до 30 секунд для реального SSO и готовности
страниц; mock-сценарии сохраняют стандартные 5 секунд. Созданные вручную browser
contexts также ограничивают navigation/action ожидания. После глобального выхода
повторный переход проверяет commit ответа и итоговый экран Central Auth, а не
событие load, которое может быть прервано корректным redirect приложения.
Точечный повтор не заменяет единый полный live-прогон без retries.

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

### Live platform gate

`SDLC_LIVE_QA=1` выбирает no-mock сценарии шести приложений, SSO, личных
токенов и реальных операций. Нужны запущенная платформа и результат
`services-base/deploy/bootstrap-qa.ps1`, включая опубликованный начальный
брендинг. `SDLC_QA_SESSION_FILE` указывает на приватный `qa-session.json`;
пароли и токены не передаются в аргументах и не включаются в trace.

```bash
SDLC_LIVE_QA=1 PLAYWRIGHT_BASE_URL=http://localhost:7722 \
  pnpm exec playwright test --project=chromium --workers=1
```

Для полного изолированного gate дополнительно задайте
`SDLC_ISOLATED_QA_PROJECT=sdlc-clean-install-qa` либо имя отдельного
`sdlc-platform-smoke-*` проекта. Linux QA-контейнеру требуется Docker socket.
Проверка отказа Auth сверяет Compose labels, останавливает только его Auth,
проверяет 503 всех шести API и обязательно запускает Auth снова, затем
проверяет восстановление сессии и отзыв PAT. `sdlc-demo` запрещён; без явного
изолированного проекта destructive-сценарий пропускается и не считается
доказательством проверки отказа Auth. Остальные live-проверки не останавливают
стенд. Full-page изображения live-набора сохраняются в `.local/screenshots`
workspace, не в исходники или Git.

### Screenshot набор

`task-header-live.spec.ts` проверяет установленный общий Header на реальных
Central Auth/Task API: 99 сочетаний трёх страниц, трёх тем и 11 ширин
320–2560 px, в том числе границы breakpoint. Измеряются высота 60 px,
порядок слотов, неперекрытие и 44/40 px целей. Дополнительно проверяются
runtime health/порядок шести UI, keyboard/touch меню, возврат фокуса,
desktop/mobile создание с контекстом, уведомления, аккаунт и подтверждённый
центральный выход с последующим отказом повторному входу без пароля.
API не перехватываются; собственный QA-проект удаляется штатным API в `finally`.
`SDLC_HEADER_EVIDENCE_DIR` позволяет выбрать приватный каталог full-page PNG
и `results.json`. Матрица `task-pages-live.spec.ts` проверяет остальные страницы,
не заменяясь этим компонентным срезом.

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
| Application | ≥90% |
| Infra (docker-тесты) | ≥85% |
| API routes | ≥85% |
| **CI gate (всё workspace)** | **≥60%** |

### Frontend

- Целевой показатель не зафиксирован в CI; приоритет — покрытие critical UI и pure utils.

## 7. Чек-лист перед merge

- [ ] `cargo fmt --all && cargo clippy --workspace --all-targets` clean
- [ ] `pnpm typecheck` clean
- [ ] `pnpm test` green
- [ ] `cargo test --workspace -- --test-threads=1` green
- [ ] `./scripts/run-e2e-tests.sh` green
- [ ] `pnpm build` green
- [ ] Playwright critical path green
- [ ] Документация обновлена

## References

- `docs/ARCHITECTURE.md`
- `docs/DEPLOYMENT.md`
- `justfile`
- `lefthook.yml`
- `backend/scripts/run-e2e-tests.sh`
