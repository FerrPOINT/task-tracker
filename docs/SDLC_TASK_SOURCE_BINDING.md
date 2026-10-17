# Task Source Binding

Кандидат контракта привязки исходников задачи. Наличие кода и сгенерированной
схемы не подтверждает установку или совместную live-приёмку с Forge.

## Owner Policy

The project owner configures GET/POST
`/api/v1/projects/{project_id}/sdlc/source-policy` through normal human SSO and
current project ownership, mapped by central `sub`. Project members/global admin
hints, email, matching local UUIDs, machine grants and agent-authored fields do
not grant policy-write authority.

POST: required nullable `expected_source_revision`, `source` with
`forge_instance_id`, `repository_id`, full `base_ref` and expected full lowercase
`base_commit`, plus `idempotency_key`. The configured namespace reader verifies
the hosted repository, exact namespace and native resolved commit. Syntax alone
is not verification. Source policy has immutable revisions/hash, CAS head and
original operation GET `/operations/{idempotency_key}`; historical GET is
`/versions/{revision}`. Exact replay does not require another external observation.

## Normal Task Flow

For NEW `single-task-v1` Drafts only, Tracker pins the project policy revision/hash
and the verified repository link in the existing Draft-creation transaction.
There is no model prompt, branch input, queue, allocator framework or additional
business confirmation step. Server ref is exactly:

```text
refs/heads/sdlc/<immutable-task-UUID>
```

The UUID determines spelling, not authority. The complete namespace/project/repo,
owner-approved base, verified full SHA and sealed binding remain mandatory.
Default/protected/base refs cannot be used as candidates. Forge must still enforce
its CURRENT union of all linked-project default/protected refs at native receive.

Existing human exact requirements confirmation/routing snapshot atomically seals
the binding. `ConfirmCommand`, old command hashes, source/assignment receipts and
the one-task/no-children contract are unchanged. No branch or pipeline is created
by confirmation. Changing project source policy affects only later new tasks;
existing selections and bindings are immutable, not rewritten to the new head.

Missing project source configuration does not block Draft creation/brainstorming.
It is recorded as an infrastructure/operator blocker, not a business question.
Confirmation cannot invent a source binding. Historical Drafts are not enrolled
automatically on GET, retry, confirmation or policy update. Missing selection
blocks confirmation rather than forcing a replacement task/chat.

## Explicit Original Draft Enrollment

GET `/api/v1/issues/{id}/sdlc/source-enrollment` reads a no-store owner preflight.
POST on the same path accepts only `expected_preflight_sha256`, `source_revision`,
`policy_hash` and `idempotency_key`. It explicitly pins the current verified,
owner-approved project source policy to the ORIGINAL task. No extra business
question, chat, ref input, runtime proof or new assignment is needed.

Admission requires exact human task-owner subject/current project access, root
single-task-v1 Draft/unconfirmed state and no confirmation/source publication,
technical assignment/execution or results in Tracker's authoritative inventory.
An active/unknown/absent PM native run does NOT block this metadata-only action.
Tracker does not inspect or change Fleet runtime, PM lease, command, input hash,
historical assignment or transcript. No Fleet HTTP producer/token forwarding,
terminal/checkpoint/negative-proof predicate or callback is involved.

Preflight CAS covers aggregate, original selection and verified policy. POST
reauthorizes and rechecks these in one transaction under the SAME issue/aggregate
lock used by confirmation. Concurrent PM edits may invalidate the aggregate CAS;
read a new preflight and retry explicitly. A successful enrollment does not stop
PM brainstorming. Same-key exact replay is metadata-only; changed input/new-key
repin and all post-confirmation enrollment/rebind are rejected.

Confirmation either rejects an unconfigured source and rolls back, or atomically
freezes the selected immutable pin. If confirmation has already committed,
enrollment rejects even original-key replay. If enrollment commits first, later
confirmation freezes that pin, never a newer project policy. No existing exact
requirement confirmation/routing checks are relaxed.

The immutable enrollment overlays only an absent/unconfigured initial selection;
original selection/state/PM assignment bytes remain unchanged. Metadata-only audit
`sdlc.draft_source.enrolled` and structured `source.selected` outbox event commit
once with the pin. Existing metadata_v1 event/context readback carries enrollment
ID, task/project/owner, policy revision/hash and sealed receipt hash, not a Git or
dispatch capability. Nothing injects a new PM run or automatically steers it.
Typed metadata projection `source.selected` зарегистрирована вместе с маршрутом
и receipt; она не запускает агента.

Initial exact requirements confirmation publishes `TaskSourceBinding`. During an
existing critical PM review, owner confirmation alone stays Draft: the trusted
PM completion publishes its new routing generation and corresponding source
binding atomically. Both use the ORIGINAL immutable task source selection;
changing project source policy never changes a running task's source. The SQL
gate accepts both original and generated snapshots through the existing routing
snapshot inventory; historical bindings remain immutable.

Pending `confirmation_revision` is the target awaiting owner confirmation, not
consent. An unconfirmed Draft may enroll after PM publishes requirements;
actual confirmation history/publication still closes enrollment.
Only a CURRENT Developer assignment/run/fence/lease after confirmation can be
considered for a scoped Forge Git grant. An active PM grant never gains branch
write rights from source selection. Published/technical-task rebind remains closed.

## Readback

GET `/api/v1/issues/{id}/sdlc/source-binding` returns tagged `state=bound|blocked`.
`blocked.reason` distinguishes missing binding/project policy/current execution,
changed source context, non-running execution and expired lease. Historical
absence is not a fake/default binding. Responses use `Cache-Control: no-store`.

The binding contains Tracker/project/task/root/confirmation/requirement/hash/
routing snapshot; namespace/Forge instance/repository/base ref/full base SHA/
exact candidate ref; source policy revision/hash and sealed binding `source_hash`.
`source_hash` is a canonical SHA256 integrity field, not a caller-issued capability.

`current` is the actual existing lifecycle readback from the SAME transaction:
assignment/execution/role/agent/hash/routing, Tracker lease/holder/version/fence and
native run/chat/machine subject with observation time. Machine read requires the
existing exact current delivery grant/subject plus project access. It does not
allow policy writes, foreign tasks or direct lifecycle commands. A retained
binding is never a substitute for current lease/fence/run verification.

`bound` is NOT a Git write credential. Forge must compare all retained operation
identities, including run/chat/subject, to current readback; only current Developer
Development/Rework may push the exact ref under native old/new CAS. Review/Test/
DevOps access does not become candidate-write access. Base issuer/crypto and
legacy Forge SAT behavior are unchanged.

## Integration Boundary

One NEW additive migration 0097 creates source policy/selection/enrollment/ref-owner/binding
history and namespace/immutability gates; no historical migration or row backfill.
Модули, маршруты, миграция и OpenAPI types зарегистрированы в приложении.
Снимок `openapi/openapi.json` получается штатным Rust-генератором; TypeScript
клиент — закреплённым `openapi-typescript`. Ручное редактирование схемы запрещено.
Новые operation ID имеют префикс `sdlc_` и не меняют операции routing-policy.

Existing Forge registered-reader GET by repository UUID proves namespace/identity,
not a source SHA or receive policy. User refs/commits GETs reuse native Git but
reject registered machine identities, accept storage names rather than public
group/slug, and do not supply the complete default/protected policy.
Forge should reuse those native operations and existing admission/policy code
behind the existing registered-reader surface. Required facts are `VerifiedSource`:
exact namespace/instance/repo/public name, approved full base ref/native commit,
authoritative default and ALL linked-project defaults/protected refs, observation
time. The source route uses the same registered-reader identity and admission;
no new auth class or broad user-route permission is added.
`ForgeReader` использует тот же зарегистрированный reader и GET
`/api/v1/namespace-repositories/{repository_id}/source` с точными `base_ref`/`base_commit`.
Caller PAT не пересылается. Redirect отключён, сжатый или превышающий 64 КиБ
ответ отклоняется; проверяются instance, namespace, repo, native SHA и время
наблюдения не старше 30 секунд. Старые catalog metadata не считаются проверкой
источника. Без настроенного reader trait сохраняет явный отказ.

Парная реализация Forge поставляется отдельно в CI/CD #122; обычный catalog
endpoint без `/source` сохраняет прежний ответ и не выдаёт source proof. Совпадение форм DTO
не заменяет её typed/HTTP/native-Git приёмку. Старая установленная версия Forge
не выдаёт фиктивный проверенный источник.

Обязательная приёмка: Rust compile/fmt/clippy, существующая проверка уникальности
OpenAPI operation ID, штатная генерация схемы/клиента и disposable PostgreSQL.
Проверяются owner/CAS/replay, запрет автоматического enrollment истории,
сохранение исходного Draft/PM assignment, гонка enrollment/confirmation,
транзакционный rollback и уникальность candidate ref. Fixture-проверки и реальные
межпродуктовые сценарии учитываются отдельно; live-приёмка не следует из unit tests.
