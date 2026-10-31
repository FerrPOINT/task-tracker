# Стратегия тестирования Task Tracker

Frontend manifest/importer больше не объявляет неиспользуемый
`i18next-http-backend`: product и Base переводы уже bundled JSON. Configs,
locales, scripts, locked versions и pins сохранены. Новый lock требует отдельной
frozen qualification; старый environment не считается проверенным этим Source audit.
[Общий consumer contract и результаты](../../services-base/docs/plans/fleet-frontend-dependency-closure-20261030.md).

## Текущий performance candidate — 31 октября 2026

Оптимизации перенесены на main `fa60e0e2` в отдельной task branch. Текущие Rust
исходники, Cargo manifests/lock, SDK `ed61ec63` и UI `03a35d80` сохранены.
Frontend preparation/CI используют Node 22.22.2, как текущие Docker recipes.
Общий порядок задаёт [принятый стандарт Base](https://github.com/FerrPOINT/services-base/blob/0a552be41d7b6586abc8cb2f54cfbccd3d7f1c59/docs/BUILD_PERFORMANCE.md).

- `just build` без scope отказывает до команд подготовки. Выбирать
  `build-backend`, `build-cli`, `build-frontend`; намеренный полный — `build-all`.
- Backend и CLI собирают только свои explicit package/bin. У них общий unit
  prerequisite; frontend `pnpm build` выполняет собственные units/typecheck/compile.
- `just test` — domain/frontend units. Полные `test-backend-all`,
  `test-frontend-all`, `gate`, coverage, HTTP/PostgreSQL и E2E — вручную.
- `api-codegen` вызывает существующий canonical postinstall wrapper с проверкой
  актуального generated output; прежней отсутствующей команды `generate:api` нет.
- Standalone CLI rustfmt не готовит SDK/Cargo target. Компиляция сохраняет exact
  SDK guard; полный Root Rust suite требует фактический `TT_TEST_DATABASE_URL`,
  ошибочная переменная Wiki из старого descriptor удалена.
- Portable selector `0a552be4` объединяет backend/CLI full suite в одном manual
  выборе; frontend full использует `test:all`. Unit subsets не запускаются второй
  раз перед полным набором. Старые Root readers могут повторять shared checks:
  Root tooling и native execution profiles в этом переносе не активированы.
- В ручном CI Rust backend и minimum-rust используют одну preparation: одинаковые
  Rust 1.88/SDK/ref/workspace проверены до объединения. MSRV cargo-команда сохранена,
  отдельный result guard принимает только success backend, включая отказ при skip.
  Timeout backend 75 минут сохраняет сумму прежних execution budgets 45+30;
  это допустимое время, не измеренная длительность или расход ресурсов.

`release.yml` вызывает текущий manual `qualification.yml`. Новые configured PM,
explicit Analysis start, legacy schema, lineage и остальные PostgreSQL controls
сохранены с текущими командами/env/conditions из main. Root workspace suite не
заменяет эти explicit ignored cases. Историческая приёмка ниже сохранена отдельно.

Scoped Source tests: 52 PASS, 4 POSIX SKIP; изменённый Just source contract затем
проверен отдельно. App/Node/Just/Rust qualification, frozen install, SDK authority,
artifact transfer, CPU/RAM/I/O/cold-warm и live — NOT RUN/NOT MEASURED. Static
path/consumer audit подтверждает scope изменения, не native compatibility.

## Configured Analysis Start: 2026-10-29

PR197 qualifies the explicit owner start, not an automatic PM transition. On
backend tree `1458e3abfd17f62aa94411a4d602dbcc3bd76987`,13 selected real
PostgreSQL scenarios cover atomic start/discovery, replay/conflict, ownership,
stale consent, rollback and populated migration history. The actual HTTP Draft
suite additionally checks denied/stale commands,201 creation,200 replay and one
intent with no assignment. Backend538 unit and44 CLI tests, workspace/all-target
check, strict Clippy, Rustfmt and Rust OpenAPI generation pass locally in Linux.
Ignored tests outside those selections are not passes. Only fresh synthetic
Compose fixtures were used, with owned resources cleaned by Base helper v2.

The frontend on pinned Base UI03a35d80 passes typecheck,724 tests across77 files,
lint, format, build and generated schema/compatibility checks. These are composed
results: the first run passed665 tests but failed to start one fork worker; the
missing file then passed59 tests separately. Five existing formatting errors
were corrected mechanically before the final format/build gates. No business UI
was changed. Generated TypeScript remains an ignored build artifact, produced
from the checked-in Rust-generated OpenAPI rather than manually maintained.

Retained result hashes and scope are in
[the evidence manifest](evidence/configured-analysis-start-20261029.json).
This is source qualification, not installation, real Auth/Hermes inference,
browser acceptance or a seven-role deployment. Those remain coordinated release
gates. PM completion alone is still tested to leave Backlog/Ready without intent.

## Быстрая разработка и ручная квалификация

Container `container_inputs` отделены от binary/Vite inputs: Dockerfile, recipe
ignore и frontend nginx выбирают упаковку сервиса, не пропуская её из-за отсутствия
application producer. Native packaging/layer reuse проверяются отдельно.

Общие правила принадлежат Base `docs/BUILD_PERFORMANCE.md` и `docs/TESTING.md`
принятого quality tooling. Product descriptor `.quality-gate.json` задаёт команды;
Automatic CI и container helper потребляют exact portable Base selector.
Root использует собственный принятый packet; его обновление не входит в этот
Source transfer и не считается выполненным по результату portable selector.
SDK/UI/skills/operator pins от quality pin независимы.

Из корня PDLC workspace после `enter-dev.ps1`:

```powershell
./check-workspace.ps1 -Profile Build -Repository task-tracker -BaseRef HEAD -PlanOnly
./check-workspace.ps1 -Profile Unit -Repository task-tracker -Component tooling -BaseRef HEAD
```

Учитываются staged/unstaged/untracked/deleted inputs. `-All` задаёт полный обход;
неизвестный путь расширяет продукт. No-change означает отсутствие выбранной работы,
не готовый image; холодную подготовку/компонент задавать явно. Docs/test-only не
запускают production producer при неизменных build inputs.

Server (`cargo build --locked --release -p server --bin server`) и официальный CLI
(`-p task-tracker-cli --bin task-tracker`) — отдельные поставляемые binaries. CLI
не добавляется в server graph. Quick Rust unit — `cargo test --locked -p domain --lib`;
CLI rustfmt остаётся static. Физическое отделение pure CLI unit от HTTP harness
ещё OPEN; domain unit его не заменяет.

Frontend `pnpm test:unit` выполняет Node source tests и Vitest без integration/
slow/e2e filename tiers. `pnpm test:all` сохраняет полный Vitest набор; Playwright
запускается отдельно. `pnpm build` выполняет unit, typecheck и compile один раз.
Оба frontend Docker recipes отделяют dependency install от OpenAPI generation:
manifest установки исключает root scripts, dependency hooks сохранены; schema и
generator копируются после install и один явный postinstall готовит types.
Source checks не доказывают реальные BuildKit cache hits или ускорение.
Общие правила — Base `docs/BUILD_PERFORMANCE.md`, раздел «Frontend, Python и Java».

Docker install использует зарегистрированный Node22/architecture download store
Base в `/pnpm/store`; integrity check включён, side-effects cache выключен.
`node_modules` остаётся локальным; реальные cache hits и ускорение пока не измерены.

CI после unit/typecheck использует `pnpm build:compile`; postinstall по-прежнему
генерирует OpenAPI types, независимый `openapi:check` сохраняет drift comparison.
Explicit app/index/Base UI Tailwind roots исключают tests/setup из production
CSS inputs. Generated types хоста не переносятся поверх prepared Docker outputs.

`python scripts/build.py` без аргументов выбирает изменённые container producers,
`backend`/`frontend` задают конкретный, `--all` — оба. При пустом выборе Base verifier
и Docker не вызываются; это не artifact readiness. Mixed SDK/UI контекст отказывает
до Docker. Raw heavy helper требует общего execution slot; Root owning command
вторым slot не оборачивать. Frozen pins ради запуска не менять.

Automatic CI выбирает компоненты как для push, так и PR; initial push явно выбирает
все. Full workspace/Clippy/PostgreSQL/migrations/MSRV/packed/themes не входят в
повседневный цикл. Полный `release.yml` вручную вызывает один reusable
`qualification.yml` с `http_only=false`. Прежний addressed input `http_only` остаётся
в ручном `qualification.yml`, вместе со всеми прежними условиями/тестами. Concurrency
принадлежит callee; caller не задаёт совпадающую группу. Это не два запуска набора.
Секрет Base передаётся по имени, без расширения permissions.

Real-Git regression старого manual classifier сохранена byte-for-byte как
`scripts/tests/test_ci_change_classifier_integration.py` и выполняется на manual
вехе. Automatic classifier проверяется pure unit через Base plan; старый
`classify_ci_changes.py` обслуживает manual/legacy compatibility.

CI download caches сохраняются; unqualified compiled targets не восстанавливаются
(`cache-targets: false`). Docker registry/git caches и корректные layers сохраняются;
compiled target свежий между независимыми exports. Данные прежних caches не удаляются.
Безопасное compiled dependency reuse, native caller/artifact handoff, реальные
Vitest/types/Vite/Rust, resource profiles и Docker freeze qualification — OPEN.
Integration/slow/release/live/8h запускаются отдельно вручную; NOT_RUN сообщать явно.

## PM Recovery: Миграция0098

В PR169 подготовлен отдельный сценарий непустого PM upgrade: fixture доходит
до0097, обычные repository/API пути создают две lease и четыре claim/heartbeat
операции. Затем реальный0098 up должен сохранить полные строки, IDs, holder,
timestamps, версии, hashes, original-key receipts и outbox; новый nullable
`recovery` проверяется отдельно. Down обязан отказать без потери истории.
Далее тот же тест проверяет recovery v2→v3 после настоящего истечения TTL.

Точный ignored selector:
`sdlc::candidates::tests::unsubmitted_recovery_tests::unsubmitted_pm_recovery_cas_witness_replay_and_history`.
Он требует свежую disposable `TT_CANDIDATES_TEST_DATABASE_URL` с суффиксом
`_candidates_test`, без migration ledger; нельзя pre-migrate или делить DB
с другим сценарием. Отдельный legacy populated test использует свежую
`TT_DRAFT_FLOW_TEST_DATABASE_URL` с суффиксом `_draft_flow_test` и теперь
проверяет0097→0098. Его данные сами по себе не доказывают PM lease history.

Workflow CI явно запускает PM selector с `--exact --ignored` в отдельной свежей
`tracker_pm_candidates_test`; она не используется legacy upgrade или HTTP fixtures.
Гейт выполняется также при addressed `workflow_dispatch` с `http_only=true`.
Проверка результата требует `1 passed; 0 failed; 0 ignored`: пустая выборка,
пропущенный ignored-тест или отсутствующая БД не дают успешную приёмку.

Локальный scoped Linux gate от 18 октября 2026 года на исходниках
`407d058a0ca4800d62f8dfa6af5d79e523429a66`: fmt, strict Clippy, typed build,
9 точных тестов и фактический OpenAPI generator проходят. В девять входят
3 app identity, 2 loopback HTTP, API schema, mock migration, оба указанных
populated PG сценария. Базы были отдельными свежими disposable fixtures;
пропуск из-за отсутствующего env не допускался. Cleanup подтверждён.

Result receipt SHA256:
`6a6ab4fe311861e5541a53be6248cf22461e81f78375c652e9ddadd180d10cb2`.
Generated OpenAPI SHA256:
`2f35bf3bbc12c3e4b711216f453c2d0f139b40c40e7ede1fcb3b0248a9e9724f`.
Retained compiler cache не является доказательством будущих проверок;
initial-cohort standalone binaries отмечены только compiled, не executed.
Server image, real Auth, установка и live Fleet/Hermes flow этим прогоном
не проверялись. Fake witness не является live runtime evidence.

Регрессии для непустой установки проверяют Todo-колонку доски, когда другие
workflow добавляют более ранние Todo-статусы в общий каталог. Счётчики доски
и бэклога должны совпадать, новая задача без спринта должна оставаться в бэклоге.
HTTP unit-тест каталога Auth включает активную учётку с пустым отображаемым
именем: вместо имени показывается email, идентичность сохраняется, отключённые
учётки исключаются. Чтение каталога не создаёт локальные профили.

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
GitHub Actions отключены. Backend, PostgreSQL и coverage проверяются локально;
контейнерные проверки используют собственный временный Compose-проект.
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

Для standalone fixture smoke с переходом в PM чат нужен явно настроенный каталог
во время сборки: `VITE_PLATFORM_SERVICES_URL=/api/v1/runtime/services pnpm build`
(PowerShell: `$env:VITE_PLATFORM_SERVICES_URL='/api/v1/runtime/services'; pnpm build`).
Затем `pnpm exec playwright test smoke.spec.ts --project=chromium` запускает свой
preview либо использует явный `PLAYWRIGHT_BASE_URL`. Сценарий перехватывает каталог
и API синтетическими ответами; без build-time URL Base по контракту оставляет
только display defaults и не разрешает переход в Fleet. Обычный production build
без этого override проверяется отдельно. Эта fixture-конфигурация не является
настройкой установленного runtime или доказательством live SSO/backend.

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

## Совместимость каталога проекта и восстановления PM

Кандидат PR170 сохраняет оба полных имени миграции 0098 и выполняет их
раздельно в populated upgrade. Отказ rollback PM проверяется до применения
каталога; затем отдельно проверяются отказ rollback каталога и idempotent up.
Пять `migration::project_repository_tests` используют разные свежие БД с
суффиксом `_pdlc4_compat_test`: legacy, published, fresh/unknown, уже применённый
каталог до PM и тот же порядок на legacy lineage. Тесты не правят ledger ради
совместимости, а сравнивают сохранённые строки и прежние применённые записи.

CI содержит явный exact/ignored gate с отдельной БД на случай и проверкой
`1 passed; 0 failed; 0 ignored`. Наличие gate не означает его успешное исполнение:
для объединённого кандидата требуется отдельный run на точном source.

Project links pagination: ignored infra integration test
`project_repository_link_paging_preserves_scope_and_order` требует явную свежую
`TT_PROJECT_LINK_TEST_DATABASE_URL` с суффиксом `_project_links_test`. CI отдельно
исполняет его с exact selector и проверяет один выполненный тест. Production
repository создаёт 102 связи, включая одинаковый repository ID в двух Forge,
и связь другого проекта; страницы 100+2 не теряют, не дублируют и не смешивают
записи, большой uint32 offset безопасен. UI проверяет переход вперёд/назад и
сброс страницы при смене проекта. Рабочие БД не используются.

HTTP fixture дополнительно проверяет 409 для создания и удаления существующей/отсутствующей
связи архивного проекта, доступность чтения и возврат к обычному идемпотентному DELETE
после следующего active generation. Fixture не уменьшает generation и не меняет lease timestamps.

## Initial PM Stops At Backlog (21 October 2026)

New source policy: exact owner confirmation publishes Backlog/Ready; accepted PM
completion separately retains the current terminal/Workflow/lease receipt.
Neither path nor replay creates an Analysis intent/event. No migration/DTO change.
The following Rust/PG checks are required for this change, not claimed executed
by the earlier 24-case bootstrap-retry qualification:

- `cargo test -p app --lib sdlc::tests`: exact consent, full revision/hash,
  zero-question complete brief, frozen PM writes, retained historical receipt.
- `cargo check -p domain -p app -p infra --all-targets` and affected Clippy.
- `cargo test -p infra --lib sdlc::candidates::tests::pm_publication_stops_at_backlog_without_analysis_on_confirmation_completion_or_replay -- --exact --ignored`:
  fresh `TT_CANDIDATES_TEST_DATABASE_URL` ending `_candidates_test`, one executed case.
- Native PM resume execution was retired with its writer routes. Preserve its
  stored history and validate the populated upgrade; do not treat removed native
  execution tests as a current configured-CLI gate.
- Changed downstream fixtures remain executable: candidate discovery, lifecycle
  heartbeat/recovery, one-task Rework, source publication and populated095 upgrade.
  Technical execution tests import a synthetic Analysis starting snapshot. They
  qualify operations from that stage, not a production PM-to-Analyst transition.
  The test-only importer accepts only owned disposable database names, requires
  actual saved configured PM completion, and uses the canonical projection/head
  writer. Only the initial canonical-transition trigger is temporarily suspended
  inside the fixture transaction; its exact enabled definition is restored before
  commit and before the tested operations. Other guards stay active. Unpermitted
  writes are rejected before and after import. No migration or production guard
  is changed, and no test is disabled or newly skipped.
- `cargo test -p server --test sdlc postgres_http_clarification_ownership_replay_gate_and_restart -- --exact`:
  isolated legacy HTTP/PG read/answer harness. Retired PM publication attempts
  must be denied; explicitly imported predecessor snapshots provide revisions,
  questions and cancellation history. Human answer/consent, ACL, rollback,
  readback and restart operations still go through HTTP. A separate synthetic
  Analysis snapshot supports retained technical admission tests. These imports
  are neither a current PM writer nor populated-history migration acceptance.
- `cargo test -p server --test drafts -- --ignored`: current configured PM HTTP
  scenario uses an ordinary assigned service, current questions, human answers,
  exact consent, final comment and authoritative Workflow readback. Consent
  stays Draft/InProgress; completion alone publishes Backlog/Ready, with no
  native lease or automatic Analyst. External Auth/Workflow/Forge observations
  are explicit fixtures, so this does not replace live cohort acceptance.

Focused UI case: `keeps confirmed requirements at Backlog without inventing
Analysis or PM completion`. Historical intent readback tests remain separate.
Use existing owned Compose/PG transport for native qualification; no live DB.

## Server image CLI metadata: 2026-10-29

Server recipes сохраняют три SDK дерева и только manifests/guard targets для
невыбранных CLI. Принятый pin: `.namespace-base-revision` / `ed61ec63`;
реальная CLI implementation и её отдельные/manual команды прежние.
Быстрый source contract: `python -B -m unittest
scripts.tests.test_ci_contract_checks.ServerCliMetadataContractTest`.
Native locked/poison/layer/old-mtime и CLI qualification — общая финальная
веха. Канонический контракт: Base `docs/plans/tracker-server-cli-metadata-20261029.md`.

Frontend dependency hooks: единственная policy — `allowBuilds.esbuild: true` в
`frontend/pnpm-workspace.yaml`. Source command: `python -B -m unittest scripts.tests.test_ci_contract_checks.PnpmLifecyclePolicyTests -q`.

Docker разделяет полный unit/typecheck context и production COPY по `frontend:build.exclude_inputs`.
Один `pnpm run postinstall` обслуживает оба consumers; `pnpm run build:compile`
зависит от успешного unit-маркера. Native cache-hit/time qualification пока не выполнена.
Unit context сохраняет `nginx.conf`, который читает routing test.
