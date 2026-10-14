# Discovery готовой single-task работы

Source-only пакет от `cd14059b8b19c81c06e11a4869dbf391743aef2a`.
Это read producer Tracker, не второй planner, reserve, claim или proof service.
Registrations/generated OpenAPI принадлежат main; до их интеграции endpoint
не объявляется доступным в runtime. Нет новой таблицы или миграции.

## Frozen Wire

`GET /api/v1/sdlc/delivery-candidates?project_id=UUID&after=UUID&limit=N`

`project_id` обязателен; `after` опционален; `limit` по умолчанию 50, от 1 до 100.
Unknown/duplicate query fields, неверные UUID/числа отклоняет штатный extractor;
недопустимые bounds отклоняет application validation. Ответ строго закрыт:

```text
contract_version: 1
tracker_instance_id: string
project_id: UUID
observed_at: UTC RFC3339 с девятью знаками наносекунд и Z
items: [{task_id, root_task_id, requirement_revision, next_operation, task_version}]
next_cursor: UUID | null
```

`root_task_id == task_id`; revision положительный. Для `ensure_queued`
`task_version` явно null, для `reserve` это actual positive lifecycle version.
Никаких assignment/run/lease refs, permission или dispatch claims в ответе нет.

## Owner Boundary

Только configured reservation scheduler с отдельным ровно `task-tracker:read`.
Human, другой subject, PM/delivery grants, write/mixed scopes запрещены.
Каждый read повторно проверяет active local user, project owner/member и opt-in
project config. ACL rows удерживаются существующим shared-lock механизмом до
завершения read; отсутствующий доступ не заменяется Central/global-admin hints.

SELECT фильтрует explicit `flow=single-task-v1`, task=root, exact latest owner
confirmation, v2 frozen routing и current revision/hash до ORDER/LIMIT.
Совместимость семи routes проверяется existing policy validator; exact snapshot
проверяется тем же helper, который сохраняет все прежние lifecycle guards.
Corrupt frozen source fail-closed с 409, не выдаёт permissive partial proof.

Initial `ensure_queued` появляется только при existing Analysis/Architecture intent
и `sdlc_current_pm_completed` текущего PM execution/current requirements. Consent,
AnalysisReady intent, завершённый чужой/старый run или owner feedback недостаточны.
Legacy Analysis hold блокирует initial seed; Architecture seed также требует
existing accepted legacy Analysis result, как существующий EnsureQueued.

`reserve` выдаётся только current Ready/Queued/unassigned head с exact frozen
routing и согласованной issue-status projection. Completed/running/reserved/unknown
и terminal heads не превращаются в новую работу. Capacity не резервируется этим
GET: существующий Reserve повторяет CAS, root/agent/pool holds и остальные guards.

UUID ordering/keyset применяется только после SQL eligibility. Запрашивается
`limit+1`; next_cursor при lookahead равен последнему **возвращённому** task UUID.
Empty/last page содержит null. Новый проход начинает без after: mutable eligibility
не означает snapshot across requests, claim или lossless event subscription.
Consumer выполняет объявленный existing command, затем fresh lifecycle readback/CAS;
не выбирает stage/role/agent сам и не трактует discovery как permit на dispatch.

Owner confirmation НЕ запускает Analyst. Fleet сначала проверяет PM final Workflow
completion и actual native terminal, затем принимает existing PM completion в
Tracker. Только matching persisted completion открывает initial discovery/seed.
Тестовые terminal observations ниже synthetic, не доказательство live execution.

## Main Integration

Main добавляет только существующие registrations:

- `domain/src/lib.rs`: `pub mod sdlc_candidates;`
- `app/src/lib.rs`: `pub mod sdlc_candidates;`
- `infra/src/sdlc.rs`: `#[path = "sdlc_candidates.rs"] mod candidates;`
- `api/src/routes/mod.rs`: объявление `sdlc_candidates`.
- Existing SDLC-authenticated v1 router: GET `/sdlc/delivery-candidates` ->
  `sdlc_candidates::delivery_candidates`.
- ApiDoc: handler и `DeliveryCandidates`, `DeliveryCandidate`, `CandidateOperation`;
  query `DeliveryCandidatesQuery` через `params`; regenerate existing OpenAPI.

Existing LifecycleRepository имеет additive default unavailable method для старых
test doubles; реальный Postgres delegate вызывает новый SELECT-only implementation.
Existing exact_revision остаётся wrapper с прежней task/owner проверкой; extracted
exact_snapshot сохраняет остальные прежние predicates без ослабления.

## Test Request

Пакет не запускает Cargo build/test, Docker или PG. Source inventory: 8 unit/API
tests и 1 ignored PG test; это не compiled collection. После main registrations
parent из `backend/` сначала проверяет ненулевой inventory через `--list`, затем запускает:

```text
cargo test -p domain sdlc_candidates::tests::
cargo test -p app sdlc_candidates::tests::
cargo test -p api sdlc_candidates::tests::
cargo test -p infra sdlc::candidates::tests::actual_owner_discovery_is_read_only_paginated_and_pm_terminal_gated -- --ignored --exact
```

Последний тест использует existing Migrator/repository/commands и fresh disposable
`TT_CANDIDATES_TEST_DATABASE_URL` с database name suffix `_candidates_test`.
Fixture сама не создаёт DB/контейнер/runner; parent владеет disposable QA lifecycle.
Покрываются consent-without-terminal exclusion, actual saved PM completion,
legacy/unconfirmed exclusions, pagination, state/ledger immutability, queue ->
reserve, stale CAS, reserved exclusion, foreign project, fresh ACL revocation и disabled config.
Native/Workflow receipts в fixture явно synthetic; DB commands и constraints реальные.

Role-artifact publisher/resolver остаётся отдельной согласуемой owner dependency.
Discovery, Workflow terminal receipt и Base evidence instructions не публикуют
analysis/design/review/test/change/check/deployment bytes и не заменяют их hash/readback.
