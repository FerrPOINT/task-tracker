# Стратегия тестирования Task Tracker

## Стабильная среда frontend-проверок

`frontend:types` и `frontend:tests` используют Base `frontend-agent-presence-v1`:
уникальные значения ID чата/сессии нормализуются в фактической среде команды,
сохраняя presence для agent-aware dependencies. Другие параметры остаются inputs.
Команда из подготовленного workspace:
`check-workspace.ps1 -Profile Fast -Repository task-tracker -Component frontend`.
Caller должен поддерживать принятый контракт; standalone CI selector закреплён
отдельным `.quality-tooling-revision`, SDK/UI pins независимы. Production artifact
и остальные gates не включены в этот opt-in. Прямой `pnpm test:unit` выполняет
новую проверку и не потребляет quality receipts.
Общий контракт и qualification boundaries —
[в Base](../../services-base/docs/plans/frontend-agent-presence-20261025.md).

## Python tooling unit

Известные `test_build_pin`, `test_verify_readme`, `test_ci_change_classifier` и
`test_tooling_ownership` выбирают только `tooling:python-helpers` в Fast/Unit/Build.
CI-contract test сохраняет свой `tooling:build-contracts`, неизвестные scripts —
полный fallback. Из root:
`check-workspace.ps1 -Profile Unit -Repository task-tracker -Component tooling -PlanOnly`.
Явный tooling выбирает оба лёгких gates. Прямой запуск из Tracker:
`python -B -m unittest scripts.tests.test_build_pin scripts.tests.test_verify_readme scripts.tests.test_ci_change_classifier scripts.tests.test_tooling_ownership -q`.
POSIX maintenance и live checks этим Python gate не подменяются.
Результаты — [в плане](plans/python-tooling-test-selection-20261024.md).

## Frontend production inputs

Ручной `pnpm test:all` также включает оба scanner/compiler controls перед одним
прежним полным Vitest-набором. Integration/slow/e2e не переносятся в test:unit.

Tailwind сканирует только src, index.html и src закреплённого Base UI. Unit/spec
и src/test исключены; e2e и tooling вне этих roots не попадают в production CSS.
Исключения production artifact согласованы с frontend_unit_inputs descriptor.
`pnpm test:unit` включает два быстрых Node tests scanner/compiler перед Vitest;
при установленной frozen среде отдельно: `node --test scripts/tailwind-source-inputs.test.mjs`.
Fixture проверяет неизменность CSS после test/tool-only edits и сохранение
легитимных application/index/UI классов. Это не full Vite или release proof.

Frontend build явно подключён к Base prepared-frontend-v1/git-source-v1.
Обновлённые source maps не дают grant прежним receipts; текущий candidate
проверяется отдельно. Node22.20.0, pnpm10.28.1, lockfile и UI pin сохранены.

## Frontend unit environments

13 проверенных pure/API-mock suites используют @vitest-environment node.
Browser setup не загружает для них DOM matchers, i18n bootstrap и storage.
Attachment, React/UI и все неразобранные suites сохраняют jsdom по умолчанию;
isolation и maxWorkers4 не менялись. Суффикс .test.ts сам по себе не означает Node.
Два controls в src/test проверяют чистую Node-среду и сохранение browser defaults.
Прежние assertions тестов не менялись; общий pnpm test:unit остаётся entrypoint.

Во время правок из root: check-workspace.ps1 -Profile Unit -Repository task-tracker
-BaseRef HEAD -PlanOnly. HEAD выбирает working-tree delta; default origin/main
включает также прежние committed изменения task branch. После commit выбирать
нужный компонент/диапазон явно, не выдавая пустой working diff за test PASS.
Test-only changes не требуют повторного production Vite build.

## Backend unit dependency graph

Обе backend recipes выбирают для unit только domain: цепочка
`manifests → unit-dependencies → unit` не компилирует server dependencies.
Full build расширяет тот же immutable слой; unit barrier, fixtures и runtime
сохраняются. Из root workspace source checks доступны через
`check-workspace.ps1 -Profile Unit -Repository task-tracker -Component tooling`.
Общие правила — Base `docs/BUILD_PERFORMANCE.md`; native qualification и замер
CPU/RAM/time нового recipe tuple выполняются отдельно. Source checks не являются
новым выполнением Rust units или acceptance прежнего image proof.

Актуальная политика 2026-10-15: unit/static запускаются при обычной сборке;
integration/slow/full coverage и browser/live — вручную перед релизом.
Команды быстрых уровней приведены в разделе «Тестовые уровни (2026-10-15)» ниже.
Прежняя полная матрица и тяжёлые команды этого документа сохранены как
ручные release checks; они не запускаются автоматически после каждой правки.

## Tracker SDLC Verification

CI explicitly runs the ignored `drafts` and `sdlc` HTTP suites against separate
clean PostgreSQL 17.6 databases in an ephemeral Actions service. The ordinary
workspace test command alone is not evidence for these suites. Ownership lease
coverage includes duplicate claims/renewals, unknown acceptance readback, fresh
ACL, expiry/restart and a heartbeat actually blocked on an issue row until after
expiry; it must return 409 without advancing the renewal cursor or history.
Lease ownership is not PM admission or genuine live Hermes acceptance.

The same `drafts` TCP/PostgreSQL suite checks PM producer publication through
the Tracker lease: human/operator credentials, a foreign machine subject, missing
PM scope, stale fencing and an expired lease are denied without state, lease or
outbox changes. Exact accepted-command replay returns the original revision,
including after lease expiry; changed-payload replay conflicts without another
write. These checks do not establish Fleet dispatch or agent execution.

The lease suite also corrupts retained assignment payloads and inserts a newer
ledger version only in its disposable fixture. Plain/operation lease GET, claim
and heartbeat must all return 409, both before and after a claim, without changing
task/issue/lease/receipt/outbox/user state. Restoring the fixture permits readback
again. This regression guards the shared current-assignment check, not lease TTL
or admission; production append-only triggers remain enabled.

`support/pm_credential_boundary.rs` is executed inside the explicit ignored
`sdlc` TCP/PostgreSQL suite. Its opaque-PAT Central fixture reproduces the child
shape (parent subject/email, no browser session, service read/write plus PM grant).
Direct legacy own/foreign issue reads/mutations and global, owner, verifier and
assignment paths must be denied before shadow linking or side effects. Bound
reads/publication recheck assignment replacement, subject, ACL revocation and
Auth outage; malformed, ambiguous or human-session PM grants fail closed.
Generic PAT/human behavior has separate unchanged-policy assertions. This is
receiving-service evidence, not real Base delegation, runtime handoff or live PM
acceptance; the verification ledger records actual runs separately.

The disposable `drafts` HTTP/PostgreSQL test also covers the strict project-access
route: sorted/deduplicated owner/member scope, read-scoped PAT, verified central
identity, global-admin/local-ID/email bypass denial, immediate membership/account
revocation, dependency failure, and the pending 0034 membership index. It uses
the real route middleware and repository, with an isolated Central Auth fixture.

`support/project_directory.rs` extends the same actual TCP/PostgreSQL harness
with the strict directory wire, mandatory null cursor, canonical IDs, UTF-8/escape
names, no extra/private metadata, read-scoped PAT and excluded admin nonmembers.
105 authorized projects exercise default 50, limits 1/7/100, UUID multipage
ordering, owner/member deduplication, bounded lookahead and empty/final pages.
Cursor replay rechecks membership/account revocation and Auth outage. Query tests
cover nil/noncanonical UUIDs, invalid bounds, duplicates and unknown fields.
The same fixture invokes `curl` over TCP: human/read-scoped PAT responses must
match the exact 200 page; unauthenticated 401 and wrong-scope 403 responses must
not disclose directory IDs or fields. This requires `curl` in the test runner.
Support-fixture rows are removed before the existing creation tests continue.
These tests require the explicit ignored-suite command below; adding coverage
does not establish an actual local PG run or live Fleet acceptance.

Focused Rust checks: `cargo test --locked -p domain project_directory` and
`cargo test --locked -p api project_directory`. API schema tests assert bearer
security, two query parameters/bounds/default and exact required response fields
(including nullable cursor). Export with the existing Rust `gen-openapi` binary,
then `pnpm generate:api` and OpenAPI drift/consumer checks; do not hand-author a
second schema or run these database tests against accepted/shared state.

Human creation requires a newly created, separate disposable PostgreSQL database
with no recorded migrations. The test refuses an already migrated database;
do not point it at accepted/shared state or reset any runtime volume.

```bash
TT_SDLC_DRAFT_TEST_DATABASE_URL=postgres://user:password@host/own_clean_draft_test \
  cargo test -p server --test drafts --locked -- --ignored --nocapture
```

This applies the final full migration chain, then tests the actual TCP HTTP route:
strict browser/central-subject/project ownership, no admin/email/local-ID bypass,
strict DTOs, transactional outbox-failure rollback, exact concurrent replay and
changed-payload conflict, forced ordinary-number collision retry, deleted-number
non-reuse, authorization revocation fencing, immutable ledger, lost response and
Tracker shutdown/restart readback. Creation itself creates no PM assignment/run. OpenAPI tests
assert all seven typed response fields, strict request fields and Draft-only enum.

The same suite also tests creation-operation GET before replay after a lost body,
UTF-8/slash path keys, unchanged seven-field result, restart/no duplicate, exact
author scope, operator/PAT impersonation denial, retained source-invalid 409 and
fresh revocation/auth-outage fencing. `support/reservation.rs` exercises initial
PM reservation over real TCP/PG: duplicate and different-key races, stale owner
CAS, immutable input checks, transaction/outbox rollback with ordinal gaps,
restart/lost response, current vs historical replay and DB/application gates
against legacy assignment and PM business writes. Selector-only/no-dispatch
semantics are asserted; no live Fleet/Workflow/native runtime is used.
This owned PG/HTTP suite requires `curl` on PATH for supplementary smoke checks
of both GET readbacks and exact reservation POST replay with synthetic credentials.

The same suite exercises immutable `pm-draft-input` snapshots: one durable UUID
and exact hash after concurrent creation/readback, replay and restart, no change
after an actual HTTP issue edit, UTF-8/CRLF composition vectors, identical context
ACL and no operator business confirmation. Missing ledger/all-null historical
input returns 409; partial snapshots and append-only mutations are rejected.
Foreign project, disabled identity, valid local HS256 token and absent/insufficient
service credentials fail closed. Inputs are not inferred from mutable issues.

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

Metadata_v1 is covered by `app::sdlc_metadata::tests` (canonical Value/UTC hash,
exact serialized UTF-8/escape thresholds, string i64 cursors, prefix boundaries,
oversized/corrupt blockers), API query/OpenAPI contract tests, and the same two
isolated HTTP/PG suites above. They cover all nine types, actual large content,
historical answer fence after edits/reassignment/restart, immutable creation
snapshot after HTTP issue edit/restart, >100 events with global sequence gaps,
count/byte paging, blocked rows and unchanged legacy/default/ACL behavior.
`TT_SDLC_METADATA_GOLDEN_DIR` optionally saves exact synthetic HTTP response bytes
as `tracker-metadata-all8.http.json` and `tracker-metadata-created.http.json` for
Fleet cross-repository parser/hash contract checks. These are generated artifacts,
not real customer input. They do not claim live Fleet/Workflow runtime acceptance.

- Каждый тест проверяет значимый путь и конкретное поведение.
- Backend: реальные интеграционные тесты с PostgreSQL через Docker; unit-тесты для domain/services.
- Frontend: unit-тесты на Vitest; E2E на Playwright.
- После изменений UI — скриншоты в 375×812, 1920×1080, 2560×1440.
- Отдельный coverage gate: строки ≥75%, регионы ≥65%, функции ≥60%
  (`cargo-llvm-cov`); полный прогон — `just test-backend-coverage`.

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

Скрипт проверяет строки ≥75%, регионы ≥65%, функции ≥60%. Эти пороги
применяются к полному workspace, включая ignored PostgreSQL-тесты.
Текущий `.github/workflows/ci.yml` запускает backend-тесты и отдельные
PostgreSQL-сценарии; coverage выполняется отдельным локальным гейтом перед merge.
Цели по слоям ниже служат ориентиром.

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
| **Coverage gate (всё workspace)** | **строки ≥75%, регионы ≥65%, функции ≥60%** |

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

## Общая база

Подключение версий, границы контрактов и проверки описаны в [BASE_INTEGRATION](BASE_INTEGRATION.md).

Legacy assignment/native admission не создаёт Tracker reservation: HTTP/PG сценарий проверяет отказ claim без lease/completion и сохраняет blocker Analysis. Успешный PM completion проверяется отдельно после настоящего reservation, финальной публикации и подтверждения exact revision. Полная интеграция с Fleet требует отдельной приёмки.

## Тестовые уровни (2026-10-15)

Обязательная общая политика: [Base TESTING](https://github.com/FerrPOINT/services-base/blob/ad724088e9a63b1e353fb5b28e4308e9ae6f45ae/docs/TESTING.md).
Выбор компонентов и native commands: `.quality-gate.json`.

### Разработка и сборка

Backend: `cargo test --locked -p domain --lib` из backend/.

SeaORM `mock` включён только в dev-dependency `infra`, а не в workspace
production features. Normal server/migration consumers сохраняют PostgreSQL,
runtime, macros и типы JSON/UUID/chrono. Существующий `infra/tests/repos_mock.rs`,
его fixtures/assertions и инвентарь Root из 59 тестов в 9 test-only файлах сохранены.
Два static TOML guard находятся в `scripts.tests.test_ci_contract_checks`:
`CiContractChecksTest.test_normal_seaorm_consumers_do_not_enable_the_mock_driver`
и `CiContractChecksTest.test_infra_mock_driver_is_owned_by_its_existing_dev_dependency`.
Они не запускают Cargo и не доказывают resolved transitive graph.

На согласованной native вехе отдельно проверить из backend/ под существующим
Base admission/fresh-target owner, при готовом cache без install/fetch:

```bash
cargo tree --locked --offline -p server -e normal,build,features -f '{p} {f}'
cargo tree --locked --offline -p migration -e normal,build,features -f '{p} {f}'
cargo build --locked --offline --release -p server --bin server
cargo test --locked --offline -p infra --test repos_mock
```

Оба normal resolved graph должны исключать SeaORM `mock`; существующий infra
test target должен получить его через dev feature и сохранить все 17 сценариев.
Ручные workspace/all-targets/test проверки могут законно объединять test features:
это не normal production graph. Полный release inventory не сужается.
Эта finite qualification пока NOT RUN; прежние Tracker59/9-case controls относятся
к test-only inputs и не повторяются ради feature split. Lockfile и независимые
SDK/UI pins прежние; экономия CPU/RAM/времени без измерения не заявляется.

Frontend из frontend/: `pnpm test:unit`, `pnpm build` включает unit.
`pnpm test` — unit alias, `pnpm test:all` — прежний полный набор.
Playwright, packed-consumer и effective-theme проверки — ручной release.

### Ручная релизная проверка

Полный прежний набор сохранён в `.github/workflows/release.yml`: только
workflow_dispatch на exact ref. Автоматический CI не поднимает PostgreSQL
и не запускает длительные/browser/live тесты.

Required security/data/compatibility checks и thresholds сохраняются.
Unit success не заменяет ручную приёмку; NOT RUN не означает PASS.

## Передача frontend artifact

CI `frontend` передаёт `tested-frontend-dist` в `frontend-package`; отдельный consumer не устанавливает Node/dependencies.
Проверка графа: `python scripts/vendor/base_frontend_artifact.py check-ci --root .`.
[Формат/команды Base](https://github.com/FerrPOINT/services-base/blob/a9b1ae9869784676957c193a75bd92a2a0480dc1/docs/BUILD_PERFORMANCE.md#передача-frontend-artifact); packet pin — `.quality-tooling-revision`.
Node 22.20.0; OpenAPI generation/checks сохраняются в producer.

Docker unit stage явно получает `frontend/nginx.conf`: routing tests читают
фактический config. Он остаётся вне Vite application COPY; изменение config
повторяет относящиеся unit и packaging, сохраняя application compilation cache.

Shell tests задают собственный feature mode: `app-shell.test.tsx` проверяет
legacy project navigation при false, `app-shell.namespace.test.tsx` — Namespace
при true. Оба входят в ordinary unit независимо от внешнего build mode; после
каждого test env восстанавливается. Нельзя трактовать legacy URLs как ожидания
Namespace UI или выключать namespace tests ради успешной сборки.
Vitest profile фиксирует default namespace=false независимо от production VITE
environment. Feature tests явно включают true через `vi.stubEnv`; production
`vite build` использует свой конфиг и реальный build flag. Это сохраняет
детерминированные legacy fixtures и проверку нового режима в одном unit graph.

## Проверка generated outputs в CI

`postinstall` выполняется при frozen install с `--ignore-scripts=false`.
Текущие outputs: `src/api/generated.ts`; они намеренно
игнорируются Git. Поэтому `git diff` не доказывает их актуальность, а требование
tracked output было ошибкой CI. Git проверяет только tracked lockfile; для outputs
проверяются ignore policy, обычный непустой файл без symlink и прежний
`pnpm openapi:check`. Проверки `test` записаны отдельными командами: failed
non-final `test` внутри `&&` не гарантирует остановку Bash с `-e`.

Повторный compiler в drift check здесь имеет конкретную причину: нет переданного
проверяемого результата generation для ignored outputs. Удалять его можно после
подключения квалифицированного postinstall handoff существующего Base owner,
с сохранением input/output identity и corruption controls. Новый cache не нужен.
Это не разрешает повторный application build или install. Backend schema drift,
OpenAPI compatibility и ручной release gate сохраняются. CI contract проверяется
штатным `base_frontend_artifact.py check-ci` до подготовки приложения.

## POSIX unit обёрток обслуживания

`maintenance:wrapper-unit` проверяет передачу аргументов `backup.sh`/`restore.sh`
через временные Python/Docker заглушки. Это быстрый Linux unit без обращения к
Base backup executor, данным или Docker daemon; настоящий backup/restore остаётся
отдельной проверкой. Команда: `python3 -B -m unittest scripts.tests.test_base_maintenance -q`.
Ubuntu CI включает модуль один раз в существующий Python tooling шаг.

Изменение только `scripts/tests/test_base_maintenance.py` в Fast/Unit/Build
выбирает этот gate без Rust/frontend preparation и artifact producers. На Windows
gate требует Linux и не выдаёт skipped тело за выполненную проверку. PlanOnly
доступен без подготовки Linux-окружения. Явный компонент `tooling` сохраняет
прежние переносимые проверки; неизвестные scripts по-прежнему расширяют scope.

## Точные Rust artifacts обычной сборки

Project-directory units модуля `sdlc` находятся в
`backend/domain/src/tests/project_directory.rs`; module остаётся под `cfg(test)`.
Оба Docker entrypoints включают файл в unit stage и исключают только его из
production COPY. Обычная команда `cargo test --locked -p domain --lib` сохраняет
прежние test names/assertions. Test-only edit этого файла выбирает unit gate,
сохраняя production artifact inputs; production neighbor по-прежнему меняет key.
Native unit/failing-unit/app-cache controls подтверждаются на общей вехе.

`backend:build` в `.quality-gate.json` выбирает только package/bin targets
поставляемого runtime: `server`.
Outputs именованы явно; отсутствие любого required executable завершает gate
ошибкой. Случайный соседний executable не попадает в artifact.
CLI `task-tracker`, migration и OpenAPI остаются отдельными consumers; их workspace/release проверки сохранены.
`backend:quality` сохраняет полные workspace clippy/tests в ручном release;
MSRV/features/OpenAPI и необходимые integration gates не сокращаются.
Fresh target между source exports и registry/compiler-cache policy Base сохраняются.

## Точный маршрут documentation validator

`scripts/verify_readme.py` и `scripts/tests/test_verify_readme.py` выбирают `tooling:python-helpers`.
Fast для source validator также сохраняет `docs:readme`; Unit/Build не выбирают application producers.
Source command: `python -B -m unittest scripts.tests.test_verify_readme`.
Mixed backend и неизвестные source paths сохраняют своих consumers; actual CI
и ручные release gates остаются прежними.

## Server image CLI metadata: 2026-10-29

Server recipes сохраняют три SDK дерева и только manifests/guard targets для
невыбранных CLI. Принятый pin: `.namespace-base-revision` / `33ce7042`;
реальная CLI implementation и её отдельные/manual команды прежние.
Быстрый source contract: `python -B -m unittest
scripts.tests.test_ci_contract_checks.ServerCliMetadataContractTest`.
Native locked/poison/layer/old-mtime и CLI qualification — общая финальная
веха. Канонический контракт: Base `docs/plans/tracker-server-cli-metadata-20261029.md`.
