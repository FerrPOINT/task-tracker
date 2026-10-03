# Task lifecycle SDLC v1

Статус: Target approved, 2026-10-02. Документ не утверждает наличие миграций или
guarded endpoint-ов. Общий регламент — services-base `docs/platform/SDLC.md`;
Tracker является владельцем Task/revisions, root/children, routing/queue/leases,
ReworkRequest, approvals, audit и transitions. Generic workflow/status APIs не
считаются реализацией этого протокола.

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
