# Task lifecycle SDLC v1

Статус: Target approved, 2026-10-02; backend B-SDLC-01 и UI B-SDLC-05 source-срезы
добавлены 2026-10-03. Runtime admission и сквозной acceptance не реализованы.
Общий регламент — services-base `docs/platform/SDLC.md`;
Tracker является владельцем Task/revisions, root/children, routing/queue/leases,
ReworkRequest, approvals, audit и transitions. Generic workflow/status APIs не
считаются реализацией этого протокола.

## Реализованный source-срез B-SDLC-01

Owner branch: `feat/pm-clarification`, существующая PM foundation PR #114.
Сверены current main `10eca7fbb47d356b22946e7e1360f2d9f0b4ef80` и открытый
пакет PR #116 `06439212829b9175e2d816c4e5f4e941e5ecea01`. Их изменения
сведены в этот checkout как owned source reconciliation; обе версии сохраняются
parents существующей reconciliation. Push и PR merge не выполняются.
Приватные role instructions/skills Base в этот репозиторий не переносились.

Существующий `POST /api/v1/issues/{id}/sdlc/requirements/{revision}/confirm`
сохраняет прежний `Confirmation` с `stage: Backlog` как receipt публикации.
После exact owner-session consent и независимого readiness evidence одна
PostgreSQL-транзакция сохраняет consent, единственный immutable Analysis intent,
aggregate `Analysis`, issue status `SDLC Analysis` (category todo), status history,
command receipt и события `requirements.confirmed`, `analysis.intent_created`.
Последнее событие имеет `event_id = intent_id`; его sequence задаёт durable order.
Analysis intent содержит `status: Ready`, backend routing `Analyst / hermes-sdlc:analyst /
analysis / business`, cycle 0 и attempt 0 (до первого admission).

В рамках среза `Clarification` остаётся совместимым внутренним состоянием
существующей PM foundation. После Analysis новые PM business commands запрещены;
исторические replay того же key/payload возвращают прежний результат с новой
проверкой owner/project ACL. Другой payload или второй ключ подтверждения дают
409. Stale revision/hash не создаёт intent. Pending миграция 000034 расширена,
вторая миграция не добавлена. Reserved PM admission gates сохранены: reservation
сама по себе не позволяет PM дойти до consent или поставить Analysis в очередь.

`GET /api/v1/issues/{id}/sdlc/analysis-intent` возвращает frozen intent только
после fresh Central service-read/project ACL; PM grant не разрешает этот ресурс.
404 означает отсутствие Analysis, 409 — отсутствующий/несогласованный retained
intent. GET сверяет intent с текущим aggregate, exact confirmation и requirements.
Чтение не claim/admission и не меняет очередь. `metadata_v1` поддерживает новое
событие; consumers должны обновить свой typed union до его потребления.

| Область | Source статус / следующий интерфейс |
| --- | --- |
| Exact consent → Analysis/Ready + outbox | Реализован backend-срез; scoped проверки ниже |
| Owner UI / typed intent и metadata consumer | Реализован B-SDLC-05; [контракт, файлы и scoped evidence](SDLC_UI_V1.md), без assignment/runtime |
| PM admission нового reserved execution | Не реализован; нужен trusted Fleet/config/Workflow binding и quiescence |
| Queue claim / execution admission | Не реализован; owner-interface gaps ниже, root exclusivity, capacity и CAS/fence до InProgress |
| До decomposition: root-only identity / queued Analysis fence | Реализован bounded guard ниже; не DAG/coverage/barrier |
| Analyst terminal → Architecture | Не реализован; accepted Workflow terminal receipt и guarded transition |
| Decomposition / children / DAG / coverage | Не реализованы; materialization owner-команды Tracker |
| Barrier / aggregate / Rework / deployment | Не реализованы; exact active revisions и trusted owner receipts |
| Сквозной PDLC / установка / acceptance | NOT RUN; этот срез не доказывает автономный runtime |

### Analysis Claim: Проверенные Interface Gaps

Проверка source owner contracts 2026-10-03: claim slice не реализован, чтобы не
вводить несовместимый assignment protocol. Workflow routing исправлен на реальный
`hermes-sdlc:analyst`; role в intent остаётся `Analyst`, callable Workflow role key
— `analyst`. Readback intent не admission, не ACK и не разрешение dispatch.
Existing `execution-lease` routes и TTL 30s / heartbeat 10s относятся только к PM
foundation; их machine grant/binding нельзя переиспользовать для Analyst.

| Owner / проверенный source | Точный gap до machine claim |
| --- | --- |
| Fleet configuration source / native admission | Main сообщил о реализованном SOURCE `GET /internal/runtime/v1/agents/{agent_id}/configuration`: fresh bounded Base PAT introspection, EXACT scopes `[fleet-control:read]`, fixed dedicated `configuration_reader_subject` и deployment-owned concrete agent UUID allowlist `configuration_reader_agent_ids`. Base issuer не выпускает compound grants; task delegation/run authority отсутствует. Ответ v1 содержит `observation_ref`, agent/role/effective revision, pinned public package metadata, observed time, `managed_files_verified:true`, но `runtime_ready:false` и native/workflow blockers. Это read-only observation, не admission/assignment ACK. Tracker client и frozen binding к intent/fence ещё отсутствуют; нужны verified native capabilities и assignment protocol. Operator actions и config proof не снимают эти gates. Этот статус получен от owner Main, не является независимым runtime acceptance Tracker. |
| Fleet `backend/api/src/routes/pm_runtime.rs`: `readback` | Trusted readback есть для PM binding/credential, не для Analyst assignment/execution/session/run. Нет callable non-PM durable acceptance lookup/ACK с exact immutable payload hash и verified-stop/terminal ACK для fencing release. Target `docs/contracts/SDLC_EXECUTION_V1.md` в Fleet PR #49 не является реализацией endpoint. |
| Workflow `project_workflow/interfaces/ui/schemas.py`: `RuntimeAssignmentRequest` | Для Analysis обязательны nonblank `decomposition_revision_ref`, `task_workspace_ref`, positive `workspace_revision` и workspace/lease generations. Tracker на этом этапе имеет confirmation + requirement revision + intent, но ещё не accepted decomposition или TaskWorkspace ledger. Нужна согласованная owner mapping pre-decomposition Analysis и logical business-only workspace; фиктивный ref/revision не evidence. Intent attempt 0 до admission; первый accepted assignment должен иметь attempt 1, не копировать 0. |
| Workflow `docs/base-sdlc-admission.md`, `application/base_admission.py` | `base-sdlc-admission/v1` принимает frozen config attestation от authenticated assignment owner, проверяет pinned source package/catalog, но не physical installation, tools/session/lease liveness. `base-sdlc-source-admission-receipt/v1` не terminal receipt и не authorization для Tracker transition/release. Нельзя считать его runtime ACK/verified stop. |
| Tracker `SdlcConfig`, `sdlc_execution_lease` и migration 000034 | Нет non-PM owner binding/grant, trusted Fleet/Workflow clients и retained Analysis assignment/capacity/ACK ledger. Intent и outbox уже durable, но это не agent/root/pool reservation. Эти строки следует добавлять только в pending 000034 после согласования minimum interfaces. |

Следующий bounded slice после закрытия gaps: Tracker как единственный assignment
producer атомарно проверяет current intent/revision и fresh machine authority,
резервирует agent 1 / root 1 / technical pool 2, сохраняет frozen assignment,
monotonic fence, lease (TTL 30s / heartbeat 10s), operation receipt и outbox.
Fleet owns execution/session/run binding и durable acceptance lookup; Workflow
owns admission/cursor receipt. Remote calls не заменяют локальную транзакцию и не
создают distributed transaction. Переход на InProgress — только после verified
concrete binding admission. Missing capabilities остаются fail-closed.

Expiry означает expired lease, а не свободную capacity: live/unknown run удерживает
agent/root/pool до trusted exact verified-stop или terminal ACK. ACK/readback
должны привязывать assignment/execution/session/run, fence и immutable hash;
caller-supplied `stopped: true`, EOF и successful HTTP status не принимаются как
доказательство. Scoped claim/capacity/heartbeat/expiry tests пока NOT RUN: endpoints
и ledger отсутствуют. Проверки ниже относятся только к implemented intent slice.

Тесты: `backend/app/src/sdlc_tests.rs`, API schema tests и изолированный
`backend/server/tests/sdlc.rs` с `support/analysis_intent.rs`. Последний проверяет
injected outbox failure/rollback, concurrent replay, payload/key/revision conflict,
immutable unique intent, ACL, запрет поздней PM revision и readback после restart.
Только disposable PostgreSQL fixture; полный интегрированный gate остаётся у
владельца общей source-вехи.

Scoped проверка 2026-10-03: PASS — app exact-confirmation test, новая app negative
metadata routing test, два API schema tests и один PostgreSQL/HTTP
clarification/replay/restart fixture с canonical workflow и Analysis assertions;
`cargo fmt --all --check` и `git diff --check` чистые. Metadata test отклоняет
старый workflow key, подмену role/mode/scope и admission attempt в queued intent.
Полные workspace build/test/clippy не запускались. Отдельные frontend scoped
проверки B-SDLC-05 перечислены в [UI evidence](SDLC_UI_V1.md).
Изолированные PostgreSQL tmpfs и Rust runner удалены; accepted/shared DB,
постоянные Compose-группы и volumes не менялись. Повторно использован существующий
QA Cargo cache; pinned Base dependency `9408802dfa978cba2f67162a49adca6f65851b01`
смонтирован read-only. Никакая role/skill content в public source не копировалась.

Локальные evidence (не включать `.local` в source commit):
`.local/b-sdlc01-evidence-7e70f162/scoped.log`,
`.local/b-sdlc01-evidence-7e70f162/openapi.json`.
Первоначальный B-SDLC-01 export имеет SHA-256
`7aa3391c1601159ceb8e92bb6eed55e11232a282e427194748bee4a540439867`.
SHA-256 текущего canonical generated OpenAPI после pre-decomposition guard:
`7a39dd79ad71687fa3f27adfc68466336a504309d0bacca6420e90c4d60f44f5`.
Штатный Rust exporter `cargo run --locked --offline -p api --bin gen-openapi`
выполнен; его результат перенесён byte-for-byte в repository `openapi/openapi.json`
по явному запросу пользователя. Schema содержит Analysis intent/readback и новое
metadata event, но не отсутствующий claim protocol. B-SDLC-05 повторил штатный
exporter после исправления query annotation `MetadataEventsQuery`; его evidence:
`.local/tracker-b-sdlc05-363ab1bf/openapi.json`. Текущий guard export:
`.local/b-sdlc01-guard-f80300c3/openapi.json`. Frontend generated client
синхронизирован штатным генератором и проверен byte-for-byte (этот файл штатно
gitignored). UI потребляет generated unions `Analysis` / `analysis.intent_created`.
Отсутствие admission/runtime
остаётся явным blocker; workflow key является строкой в DTO, его canonical
значение enforced application/projection/SQL, а не OpenAPI enum.

### Файлы Для Интеграции

Все пути ниже относительно этого owner checkout. Подготовленная reconciliation
содержит обоих parents, конфликтов нет; исходный owner HEAD перед source commit:
`3d9640b60b2c88f66a499cc6752c28583714bf9a`. Source commit объединяет verified
B-SDLC-01/B-SDLC-05/guard и owned package/main changes; full gate остаётся отдельным.
Опубликованная PR114 не обновляется, push/ready/PR merge не выполняются.

| Группа | Точные файлы |
| --- | --- |
| Domain | `backend/domain/src/sdlc.rs`, `backend/domain/src/sdlc_metadata.rs` |
| Application | `backend/app/src/sdlc.rs`, `backend/app/src/sdlc_metadata.rs` |
| Repository / pending migration | `backend/infra/src/sdlc.rs`, `backend/migration/src/m20261001_0000034_sdlc_clarification.rs` |
| API / OpenAPI annotations | `backend/api/src/routes/sdlc.rs`, `backend/api/src/lib.rs` |
| Generated OpenAPI | `openapi/openapi.json` (штатный exporter, без ручных schema edits) |
| Tests | `backend/app/src/sdlc_tests.rs`, `backend/server/tests/sdlc.rs`, `backend/server/tests/support/metadata.rs`, новый `backend/server/tests/support/analysis_intent.rs` |
| Slice docs | `docs/API.md`, `docs/ARCHITECTURE.md`, `docs/CHAT_CLARIFICATION_CONTRACT.md`, `docs/DATA_MODEL.md`, `docs/SDLC_LIFECYCLE_V1.md`, `docs/TZ.md` |
| UI B-SDLC-05 | Точный дополнительный список frontend файлов и tests: [SDLC_UI_V1.md](SDLC_UI_V1.md) |
| Imported main/package docs | `docs/CLI.md`, `docs/CLI_INSTALL.md`, `docs/CLI_VALIDATION.md`, `docs/WORKFLOW.md` |

## Pre-Decomposition Guard B-SDLC-01

2026-10-03: Main выбрал bounded fallback после сверки owner prerequisites:
ближайший concrete lifecycle guard вместо speculative Architect API.
`Stage` пока не содержит Architecture;
нет non-PM admitted assignment/fence/config ledger, accepted Analyst terminal
transition или authenticated Architect materialization/terminal capability.
Оператор, PM credential, Base source-admission receipt и caller-supplied flags
не заменяют эти prerequisites. Поэтому parent/children/dependencies, versioned
coverage, dependency cycle validation и root barrier не объявляются реализованными.

Закрыт существовавший обход: `POST /binding` раньше принимал другой same-project
root UUID и позволял claimed child войти в самостоятельный PM/Analysis pipeline
без accepted decomposition. Теперь команда требует `root_task_id == route id`
после exact human owner/project ACL и под прежним issue lock. Другой, nil или
missing root возвращает 422 без state/history/idempotency/outbox writes.
Сам task должен существовать, быть live и доступным. Replay настоящего root
сохраняется; topology/reparenting не создаются и не эмулируются generic links.

`TaskState::require_root` и repository load отвергают retained non-root/nil task
identity с 409; application reads/commands и consent readiness также fail-closed.
Pending 000034 получает только root-only CHECK и `sdlc_analysis_gate`: changed
aggregate/control row после queued Analysis запрещён до future guarded admission.
Нельзя откатить stage к Backlog/Draft, подменить revision/hash/assignment либо
добавить invented accepted decomposition и затем обойти issue status gate.
No-op state update и обычный frozen readback остаются допустимыми. Новых assignments,
receipts, producer/scheduler, авторуна или миграции 000035 нет.

Future Architect slice должен согласовать accepted Analyst -> Architecture
transition, actual non-PM admitted identity/fence и trusted terminal authority.
Только вместе с этим root-only CHECK можно заменить на реальные ROOT/DELIVERY_CHILD
membership/parent/project/revision constraints, atomic materialization, DAG/coverage
validation и barrier по exact trusted DevOps deployment/acceptance receipts всех
обязательных children активной revision. Empty/Done flags не evidence; root должен
пройти собственный aggregate и Deployment/acceptance. До этого нет endpoint, который
мог бы открыть barrier, и не создаётся always-success/empty graph implementation.

Файлы этого дополнительного guard относительно owner checkout:

| Группа | Точные файлы |
| --- | --- |
| Domain/application/repository | `backend/domain/src/sdlc.rs`, `backend/app/src/sdlc.rs`, `backend/infra/src/sdlc.rs` |
| Strict existing API / schema test | `backend/api/src/routes/sdlc.rs`, `backend/api/src/lib.rs` |
| Единственная pending migration | `backend/migration/src/m20261001_0000034_sdlc_clarification.rs` |
| Tests | `backend/app/src/sdlc_tests.rs`, `backend/server/tests/sdlc.rs`, `backend/server/tests/support/analysis_intent.rs`, новый `backend/server/tests/support/lifecycle_guard.rs` |
| Docs | `docs/API.md`, `docs/ARCHITECTURE.md`, `docs/DATA_MODEL.md`, `docs/TZ.md`, `docs/CHAT_CLARIFICATION_CONTRACT.md`, этот документ |
| Generated schema | `openapi/openapi.json` через штатный exporter; `frontend/src/api/generated.ts` через штатный генератор (gitignored) |

Scoped проверки: PASS, семь targeted Rust tests (три app, три API schema, один
isolated PG/HTTP fixture), `cargo fmt --all --check`, штатный offline exporter,
frontend generated API byte consistency и scoped pinned Base typecheck.
Tests не создают Architect assignments/receipts: они проверяют
concurrent same-project/cyclic-root binding rejection без writes, DB child-row
CHECK, stage/revision/assignment/invented-decomposition mutation refusal, прежний
atomic confirmation/outbox, replay и restart readback. Полные builds/CI не запущены.
Evidence: `.local/b-sdlc01-guard-f80300c3/scoped.log` и `openapi.json`.

QA helper `.local/b-sdlc01-scoped.ps1` переведён с прямого docker runner на настоящий
Compose `.local/b-sdlc01-guard.compose.yml`: temporary
`sdlc-qa-b-sdlc01-guard-<unique>`, labels `sdlc.task`/`sdlc.purpose`, internal network,
PostgreSQL tmpfs, no host ports и `finally down --remove-orphans`. Следующий запуск
проверяет ownership и убирает собственный exact project после interruption.
External shared Cargo/target/rustup caches сохраняются, чужие containers/volumes
не удаляются. Root `audit_docker_groups.py` во время проверки: PASS, violations [].
Accepted/shared DB и Fleet/Base/Forge исходники не менялись. Эти guards проверяются
на fresh disposable schema; rollout/data audit существующих non-root rows или уже
применённой 000034 остаётся интеграционным prerequisite, не поводом для reset/backfill.
Runner завершился, `finally down` удалил свой PostgreSQL container и network;
Compose checks container удалён через `run --rm`. Остаточные container/network/volume
lookup по exact Compose project пустые. Повторный root Docker audit: PASS,
violations []. Shared caches сохранены. Source commit после scoped checks не
заменяет full gate; push/ready/PR merge не выполняются. Reconciliation parents
main/PR116 и предыдущие PM foundation/B-SDLC-01/B-SDLC-05 source edits сохранены.

## Модель и persisted invariants

Девять стадий: Draft, Backlog, Analysis, Architecture, Development, Review,
Testing, Rework, Deployment. Операционные статусы Ready/InProgress/Completed
независимы от stage. В UI: Готово/В работе/Завершено. Waiting/blocked/queued —
derived scheduler states, не новая стадия и не четвёртый операционный статус.

Target aggregates (конкретные SQL migrations — дальнейшая реализация):

| Объект | Владелец / ключи и ограничения |
| --- | --- |
| Task | immutable tenant/project/task/root ID, kind ROOT/DELIVERY_CHILD, stage/status/revision |
| RequirementRevision | frozen requirements/criteria, sources, hashes, exact user confirmation |
| DecompositionRevision | root, requirement revision, настоящие child IDs, dependencies, coverage, accepted revision |
| TaskWorkspace | logical root workspace; root + active children; concurrency 1 |
| QueueItem | task/stage revision/scope/cycle, stable order, attempt, operationKey, prerequisites |
| AssignmentLease | assignment/agent/config binding, fencing token, TTL 30s, heartbeat 10s |
| ReworkRequest | source Review/Testing, immutable findings/input refs, target task/cycle |
| Approval / Clarification | Разные requests с exact revision/run/scope и single-use response |
| Outbox / Inbox / Audit | Transition и intent одной транзакцией; immutable event/key/hash и replay ledger |

Иерархия и dependency graph не имеют циклов. Направление dependency:
dependent child → prerequisite child. Rework stage cycle допустим и не входит
в dependency DAG. Child одного проекта/root/active decomposition; missing,
orphan, cross-project и ambiguous links отклоняются. Все созданные обязательные
children должны иметь criteria и покрывать root requirements; пустая принятая
декомпозиция не открывает barrier.

## Начало и декомпозиция

PM формирует Draft/InProgress; имя 3–4 смысловых слова, до 80 символов.
Guarded updateDraft ограничен metadata текущего Draft и same-project ссылками;
project/kind/stage/status нельзя менять generic patch. Expected revision/CAS,
operation key и проверка role permission обязательны. Existing attachment refs
можно читать/связывать, бинарные вложения не выдумываются.

`publishDraft` принимает только confirmation exact requirement/Task revision.
В одной транзакции Draft → Backlog, default neutral status, audit и outbox.
Backend автоматически создаёт единственный Analysis intent и переход
Backlog → Analysis/Ready; PM не dispatch-ит агента. Redelivery/restart не удваивают
assignment или публикацию. Stale confirmation отвергается и требует нового показа.

Analyst сохраняет requirements/criteria. Architect materialize-ит настоящие children
с dependency DAG, frozen scope/allowed paths и coverage. После accepted report и
accepted decomposition его run завершается. Root остаётся Architecture/InProgress
с derived waiting-children, **без активного Architect assignment/run**. Children
начинают собственный Development/Ready после готовности dependencies; PM/Analysis/
Architecture root не повторяются для child без нового принятого requirement scope.

## Queue и barrier

Tracker хранит очередь; Fleet не создаёт конкурирующий scheduler. Logical workspace
root сериализует children и aggregate: не более одного starting/running execution.
Общий pool по умолчанию 2, concrete agent capacity 1; PM вне technical pool, но
имеет durable assignment и root exclusivity. Non-PM admission резервирует logical
capacity; конкретный filesystem workspace предоставляет Forge только если нужен.
Dependency-ready child выбирается детерминированно по сохранённому priority/order;
не моделью. Нет capacity — очередь без изменения на InProgress до accept binding.

Barrier проверяется атомарно на latest root/decomposition/requirements revision:
все обязательные children этой revision завершили DevOps passed с trusted exact
deployment + acceptance receipt. Done flag без evidence, cancelled/failed/stale,
reopened и retired children не засчитываются. Изменение декомпозиции/требований,
reopen или stale receipt закрывает barrier и fence-ит уже выдаваемые aggregate
assignments; active run останавливает владелец перед новым запуском, не просто TTL.
Accepted history/evidence не переписывается. После barrier root получает собственный
Development/Ready aggregate → Review → Testing → Deployment/Completed.
Root завершает собственный Deployment + acceptance, не rollup children.

## Terminal transitions

`completeAssignedStage(sessionId, runId, expectedTaskRevision, outcome,
findings?, operationKey)` — название target capability, не CLI-команда.
Дополнительно проверяются assignment/execution/fence/config/input/receipt hashes.
Runtime successful event сам по себе не меняет Task. Нужен accepted Workflow
complete=true receipt, trusted Forge/evidence gates и required approval.

| Назначенная стадия | Outcome | Действие владельца |
| --- | --- | --- |
| Analysis | passed | Architecture/Ready |
| Architecture root | passed | Принять декомпозицию; waiting children без Architect run |
| Development | passed | Review/Ready |
| Review | passed | Testing/Ready |
| Testing | passed | Deployment/Ready |
| Review или Testing | needs_rework | Frozen ReworkRequest, следующий cycle, Rework/Ready |
| Rework | passed | Review/Ready, затем Testing; прежние проверки не пропускать |
| Deployment child | passed | Child Completed, проверить barrier |
| Deployment root | passed | Root Completed только с собственным acceptance |

Rework требует минимум один finding: severity, location/ref, reproduction,
expected/actual и evidence. Role/mode/stage не передаются произвольно агентом.
Developer initial/rework — один workflow; delivery/aggregate — независимый scope.
Повтор operation key/payload возвращает прежний receipt. Новый payload того же key
conflict. Retry увеличивает attempt, не cycle. После трёх автоматических Rework
cycles следующий запрос эскалируется человеку и не создаёт cycle 4 автоматически.
Ни stale result, ни поздний ответ не меняют новую revision. Human approval блокирует
автоматический переход до exact-scoped подтверждения; clarification не approval.

Transition, ReworkRequest и Tracker outbox event атомарны в его БД. Межсервисные
ACK/lookup дают eventual consistency; distributed transaction/shared schema нет.
Lease expiry не даёт права повторить dispatch, пока прежний run не подтверждённо
прекращён. Owner release только после durable terminal ACK или safe checkpoint.

## Scope прав и негативные сценарии

Backend выводит context из Task; read context token не разрешает mutation/terminal.
PM получает только Draft capabilities, Architect decomposition capability,
Reviewer/Tester свои outcomes, Developer branch write в Forge, DevOps scoped
delivery policy. Generic human API не позволяет impersonate agent или bypass gates.
Cross-tenant Task/child/receipt lookup не раскрывает title/count/activity/mode.

Проверки реализации: exact confirmation и auto pickup; duplicate publish/claim;
atomic completion и crash между commit/ACK; empty decomposition/coverage gap;
child dependency cycle; reopened/stale/retired child barrier; root own acceptance;
findings empty/late; retry vs cycle; cycle limit; insufficient capacity; TTL с
живым run; stale fences; mode injection; context token mutation; permission leak.
Текущие generic CRUD/status/links переиспользуются; новые guarded lifecycle команды
не заменять автоматизацией UI или обычным issue transition.
