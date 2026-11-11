# Стратегия тестирования Task Tracker

## Current Execution And Reviewer Handoff

The 2026-11-11 source regressions cover the current delivery execution key after
reservation/retry, absent delivery authority before reservation, preserved PM
history, and safe typed Start/proposal errors. Agent Start, Completed and
application-owned next-stage/Ready are different actions.

Real PostgreSQL selectors in the existing lifecycle fixture cover shared
Reviewer/Tester rework cycles, Tester cap, Review -> Testing -> Deployment,
and both archived Review verdicts after117. The registered migration tests
exercise Published and ProjectRepository predecessors116, preserve ledger
timestamps, normalize CRLF function definitions and prove repeated up is inert.
Each ignored selector requires its own fresh *_lifecycle_test or
*_pdlc4_compat_test database and a verified nonzero test count.

Archive readback must not run the current transition policy again or create
another assignment. New completion still requires exact current proof and
must not accept an archived terminal Review shape. Model/Advisor inputs and
criteria are unchanged; these mechanical checks are not live-agent acceptance.

The earlier frozen tree `fc3301f166e437d096cd934851d0b5a7b45959ef` passed all12 selected
local source gates:841 backend native tests, eight exact PostgreSQL scenarios,
strict workspace/all-target backend Clippy, Rustfmt and actual Rust OpenAPI
generation. The native run has112 ignored tests, not counted as passes; each
selected PostgreSQL gate separately reports exactly one pass. API72 includes
all three `stage_start_*` regressions.

Receipt: `sdlc-qa-tracker-status-b2467d46d8ef-result.json`, `passed=true`,
`cleanup_verified=true`, journal `cleaned`. Backend SDK is `ed61ec63`; independent
CLI supplier is `5f5bcb5e`. QA uses Rust1.88.0, debug0/incremental0 and that
toolchain's bundled LLD; production profiles and dependencies are unchanged.
Those results precede forward118 and its fixture regressions; they do not
qualify the later SQL change. Current qualification is recorded separately below.

The generated572588-byte OpenAPI matches the frozen tracked blob and checkout
byte-for-byte: SHA256
`ad30dd5f67443b342f7031f0f94361cd0a5d64dac159407095f9985b88291902`.
CLI64 and frontend897, typecheck/lint/build are reused from unchanged sources
and pins qualified in [PR214](https://github.com/FerrPOINT/task-tracker/pull/214).
They are not new UI or live checks. Hosted Actions, installation and seven-agent
runtime acceptance have not been performed for this candidate.

The browser fixture is separately qualified on tree
`c36ef772ce1b778c15e58ed58d8d5f2bd57bd6e0`: all six Chromium smoke cases pass
with one worker and zero retries. The SSO fixture supplies separate signed
access/ID tokens and the pinned UI's typed application-access decision; project
administration cases use a consistent administrative actor. Selectors follow
the actual accessible controls. Product authorization and all six scenarios'
business assertions are unchanged.

Receipt `tracker217-playwright-eed2fa640d3c/result.json` records format, lint,
fixture typecheck/build, six executed tests, screenshots at375/1920/2560,
unchanged source inputs, closed preview port and verified scratch cleanup.
This is synthetic browser qualification, not live Auth/backend acceptance.
### Final Composite Qualification

Frozen tree `5cd1a8629b50422039fe7d8c9280fcaadf6fa42e` combines 842 native
backend passes, 64 CLI passes and 114 distinct PostgreSQL passes. The 112 unchanged
PG cases retain their executed evidence; notification and the updated HTTP SDLC
case each have a new exact pass. This is a composite qualification, not a claim
that an earlier failed full command passed. Both118 upgrade lineages are included.

Only four server-test files changed after the instrumented baseline. Compiler
dependency metadata and 214 unchanged ELF checksums prove the reuse boundary.
Superseded test objects and raw profiles were removed by LLVM binary identity
before rebuilding the changed case. Its settled historical fixture preserves
GET 200/ACL/no-write/restart coverage without restoring retired HTTP writers;
ledger and receipt timestamps use the same exact PostgreSQL clock value.

Standard LLVM coverage is 61950/68525 lines (90.40496%), 91717/103990 regions
(88.19790%) and 4564/6017 functions (75.85175%), above the existing 75/65/60 gates.
The original five exclusions remain; only the external `/src/services-base/`
supplier prefix is additionally excluded. No Tracker source is newly excluded.
Strict workspace/all-target backend Clippy and Rustfmt pass.
Fresh Rust OpenAPI generation matches the tracked 572588 bytes exactly, SHA256
`ad30dd5f67443b342f7031f0f94361cd0a5d64dac159407095f9985b88291902`.

Receipt: `tracker217-final-5cd1a8629b50-qualified-result.json`. Earlier failures
remain separate evidence. No model calls, runtime installation or live-agent
acceptance are claimed by these source checks.

### Plan Content Authorization

Forward118 restores the plan-table dispatch overwritten by native-artifacts113.
The existing `content_writes_require_current_actor_role_and_active_admitted_space_through_commit`
case is the ordinary repository regression: valid active-space writes must pass
without admitting a foreign actor or an archived-only space. Do not change the
fixture's role to bypass a dispatch error.

`native_artifact_content_port_is_exact_and_rechecked_at_commit` exercises both
plan tables under active admission and the existing native scope, including
foreign-task/author and read-only refusals. Its sealed attachment INSERT and
deferred commit checks remain separate. These direct SQL-port assertions do not
claim a complete live Architect run.

The two118 upgrade cases require separate fresh owned *_pdlc4_compat_test
databases. Both registered lineages must preserve the function OID and previous
ledger entries and make a repeated up inert. Historical migration SQL is not
rewritten. The newly changed SQL requires fresh qualification; the earlier841/8
source results above are not evidence for118.

## Agent Stage Status: 2026-11-07

Source qualification covers explicit agent Start/InProgress, recorded Completed
and application-owned next-stage/Ready. PM Draft/Completed hands off through the
existing atomic Analysis queue; delivery requires native terminal verification.
Final DevOps and rework-limit outcomes without handoff retain Completed. Owner
consent, assignment/fence checks, exact replay and preserved history remain gates.
There is no new autoAdvance flag or Analyst-disable requirement for acceptance;
Analysis/Ready is the normal handoff and the Analyst may subsequently run.

Qualification is composite, not one green full run. Frozen tree `c155a6b9`
passes827 native backend tests,63 CLI tests and24 selected real PostgreSQL/HTTP
scenarios, including all delivery roles, PM handoff, Rework, decomposition and
populated history. Strict backend/CLI Clippy, Rustfmt and actual Rust OpenAPI
generation pass. The generated OpenAPI exactly matches the checked-in file.
Receipt: `sdlc-qa-tracker-status-8f690dafafa2-result.json`; SDK `ed61ec63`,
independent CLI supplier `a657f51f`, owned cleanup verified.

Forward114 tree `dad19419` separately passes its native-scope PostgreSQL regression,
PM explicit Start/handoff and terminal readback with the combined114→115 registry,
strict backend Clippy and Rustfmt. Receipt:
`sdlc-qa-tracker-status-b75ca7495316-result.json`; owned cleanup verified.
All897 frontend tests and typecheck pass on the unchanged frontend tree of main.
Unselected ignored tests are not passes. Earlier failures retain their original
results. Source qualification used no model calls; installation and live-agent
acceptance remain open. The dated section below records the previous contract.

Новые Architect proposal/Complete требуют сохранённую decomposition revision с
точным hash и requirements binding даже до первого плана. Регрессия
`decomposition_architect_requires_current_role_fence_and_requirements` проверяет
отказ opaque/missing references без изменений состояния; успешные all-role,
rework и critical fixtures сохраняют план через repository API. На исходниках
`0ad2841c8820d3040ad0402b9b648290532f6d5e` пройдены девять адресных
PostgreSQL-сценариев (Architect, все роли, Rework, ограничения Reviewer/Tester и
четыре critical-сценария), полный backend-набор и Clippy с запретом warnings.
Workspace rustfmt проверен отдельно. Временные QA-ресурсы убраны; вызовов модели
не было. Установка и живая приёмка Analyst/Architect остаются отдельными gates.

## Native Retained Read Scope: Forward114

Существующий тест требует свежую собственную disposable PostgreSQL DB без таблицы `projects`;
`TT_NATIVE_SCOPE_TEST_DATABASE_URL` указывает имя с суффиксом `_native_scope_test`.
Не использовать рабочую БД. Из `backend` внутри штатного QA runner:

```sh
cargo test --locked -p migration --lib \
  m20261106_0000114_native_read_scope_attenuation::tests::retained_reads_attenuate_mutation_without_granting_writes \
  -- --exact --ignored --test-threads=1
```

Регрессия воспроизводит прежний отказ и проверяет attenuation, запрет escalation,
SDLC/content/status writes из read-scope, identity/clock, ACL и archive. Она
исполняет реальные guard-функции на узком SQL fixture, не весь registry114→115
и не живой PM completion. Нужен ровно `1 passed; 0 failed; 0 ignored`;
этот результат получен на `dad19419`. Совместимость registry и PM handoff проверена
отдельными сценариями выше; живая приёмка PM этим не подменяется.

## Configured Analysis Start: 2026-10-29

Historical qualification of the previous contract, superseded above.

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

## Historical: Initial PM Stops At Backlog (21 October 2026)

Superseded by the explicit agent status and system-owned handoff contract above.
The following records the earlier source qualification, not current routing.

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
