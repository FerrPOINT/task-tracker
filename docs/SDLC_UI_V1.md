# Owner SDLC UI v1

## Merged PDLC Qualification: 15 October 2026

Merged baseline `efe1c0172df953fbcfb221766bd00095f540cedc` includes the Draft
confirmation fix and actual PDLC `DescriptionItem` fields. UI, SDK and namespace
pins remain **`7c7b24cd57cb4470b696d97933283018a20fbbb3`**. Package is `@pdlc/ui`
from `base-modules/ui`, not the old `@sdlc/ui`/Base8e cache.

The merged panel failed its initial Prettier check. The sole permitted source
adjustment is `a545a5f6fb97457d85bd49d8be4db1dbab6dacd6`: conditional source-profile
props wrapped onto separate lines, no semantic change. All qualification below
ran after that formatting correction and matches the committed source hashes.
It is not a claim that the unformatted baseline passed format checking.

Policy query, refresh, render and submit share the confirmation-context helper.
Draft `awaiting_confirmation` and `clarification` require actual server
`can_confirm`; required-open-question/read-only/stale contexts cannot submit.
Brainstorming cannot confirm. Legacy Clarification and Tracker's server-confirmable
legacy Draft projection without a submode remain compatible. Owner/questions/
evidence authority is Tracker's `owner(actor) && ready()`, not an inferred status,
waiting reason or comparison against a local user ID.

### Actual Gates On The New Inputs

- Focused API/panel/context helper Vitest: **156/156 PASS**, three files.
- Whole frontend Vitest: **489/489 PASS**, 56 files, one full run/one worker.
- Full frontend typecheck, strict ESLint plus PDLC semantic classes, and full
  frontend Prettier check: PASS.
- API generation and current `@pdlc/ui` OpenAPI drift check: PASS.
- Full `pnpm build`: PASS, one run.
- Current Draft browser fixture: **9/9 PASS**, Chromium/Firefox/WebKit at
  375x812, 1920x1080 and 2560x1440.

Dependencies were installed from the unchanged frozen Tracker lock into an own
temporary store with lifecycle scripts disabled. All 352 installed UI package
files were hash-checked against the exact Base Git export; public exports were
present. No old cache substitution, node_modules copy, shared cache writes,
dependency update, pin change, Cargo/Docker or main-checkout write occurred.
An initial private runner option error stopped before installation/tests; its
failed log and cleanup receipt remain preserved, not counted as passing gates.

### Current Full-Page Fixture

The actual production issue-detail SDLC tab receives a typed Draft
`awaiting_confirmation` owner context, full requirements revision2, concrete PM
assignment and declared v2 policy through intercepted fixture reads. Only
references are expanded. Both consent/publication checkboxes remain unchecked;
confirmation stays disabled. Business writes, JS errors and unmocked origins: zero.
All seven roles and 378 heading/reference bounds are checked without horizontal
overflow. No frozen routing snapshot is fabricated before publication.

The [new relative-path manifest](assets/screens/sdlc-pdlc-draft-confirmation-20261015-efe1c01/manifest.json)
records exact source/pins, qualification receipts, viewport versus actual PNG
dimensions and SHA256. Captures use fullPage=true/scale=css; pixels were not edited
or resized. Actual PNG sizes at viewport375/1920/2560 respectively:

| Browser | Actual PNG |
| --- | --- |
| Chromium | 375x6243 / 1920x4019 / 2560x4019 |
| Firefox | 375x6228 / 1920x4024 / 2560x4024 |
| WebKit | 375x6223 / 1920x4019 / 2560x4019 |

Visually inspected [Chromium mobile](assets/screens/sdlc-pdlc-draft-confirmation-20261015-efe1c01/chromium-375x812-fullpage.png),
[Firefox desktop](assets/screens/sdlc-pdlc-draft-confirmation-20261015-efe1c01/firefox-1920x1080-fullpage.png)
and [WebKit2560](assets/screens/sdlc-pdlc-draft-confirmation-20261015-efe1c01/webkit-2560x1440-fullpage.png).
Scoped references and consent controls have no clipping/overlap. This does not
qualify unrelated issue editing controls or production namespace/SSO rollout.

**Fixture-only, not live SSO, publication or seven-role acceptance.** Own preview,
junctions, node_modules, store and Git export were removed; lock bytes unchanged.
Earlier source36f7ce9/docsd3c4f5b images on Base8e remain untouched historical
evidence, explicitly NOT qualification of these PDLC inputs. Older Clarification
captures and the sections below retain their original source/legacy scope.

## Routing Output v1/v2: 15 October 2026

The current frontend consumer explicitly validates both existing output families.
Legacy v1 retains its exact required package commit/manifest and catalog3 checks.
Single-task v2 requires `instruction_profile:single-task-v1`, matching instruction
and catalog digests, exact seven-role mappings and catalog1; package fields must
be omitted, not null or fabricated. Mixed families, missing source fields,
duplicate identities and false readiness/admission claims fail closed.
Generated API types remain the source, refined locally without casts or DTO edits.

An Analysis intent now reads the existing task `routing-snapshot` endpoint and
matches its policy family, task/root/project/instance, confirmation and requirements
pin before accepting the Analyst workflow. It never substitutes current project
defaults. Snapshot404 preserves only an un-routed legacy intent; a profile intent
without its frozen snapshot, mismatches and other read failures remain errors.
The UI displays package references for v1 or instruction references for v2,
without granting native readiness, dispatch or owner confirmation.

Consumer source `03c9a1e` integrated as `2235c5d` passes **133 focused tests** in
`src/api/sdlc.test.ts` and `src/features/sdlc/ui/SdlcPanel.test.tsx`, full frontend
typecheck, focused ESLint/Prettier and generated-client OpenAPI check. That verified
frontend closure is reused here, not rerun or represented as new test execution.
OpenAPI/SDK structural compatibility is separate; no blanket exception is added.

### Historical Legacy Clarification Publication Fixture

Source `2235c5d76c092b7d7da29f71aaa1245eec7862c3`, Base UI
`8e944aa83c951008c227e28d9df15ed40719abb1`: **9/9 Playwright cases PASS**,
Chromium/Firefox/WebKit at 375x812, 1920x1080 and 2560x1440. This uses the actual
issue-detail SDLC tab with synthetic signed SSO and intercepted API reads,
Clarification `can_confirm:true`, all ten requirements fields and a declared v2
policy. Only the references disclosure is opened. Both consent/publication
checkboxes remain unchecked and confirmation stays disabled; no confirmation or
other business write occurs. No task routing snapshot is fabricated before
publication; that Analysis-only read is not exercised by this Clarification case.

All seven roles' actual agent/config/instruction/workflow/catalog references are
checked, package rows are absent, and 378 heading/reference bounds checks show no
horizontal overflow. JS errors and unmocked origins: zero. Full-page PNGs use
`scale:css`, without resizing or pixel edits; viewport and actual image dimensions
are distinct in the [generated relative-path manifest](assets/screens/sdlc-routing-v2-owner-20261015-2235c5d/manifest.json).

| Browser | Actual PNG at viewport375 / 1920 / 2560 |
| --- | --- |
| Chromium | 375x6223 / 1920x4019 / 2560x4019 |
| Firefox | 375x6208 / 1920x4024 / 2560x4024 |
| WebKit | 375x6203 / 1920x4019 / 2560x4019 |

Visually inspected [Chromium mobile](assets/screens/sdlc-routing-v2-owner-20261015-2235c5d/chromium-375x812-fullpage.png),
[Firefox desktop](assets/screens/sdlc-routing-v2-owner-20261015-2235c5d/firefox-1920x1080-fullpage.png)
and [WebKit2560](assets/screens/sdlc-routing-v2-owner-20261015-2235c5d/webkit-2560x1440-fullpage.png).
Scoped references have no clipping/overlap. This does not qualify unrelated issue
editing controls. V1 control coverage is the reused focused tests, not a new v1
browser case. Earlier 18 viewport and 9 full-page event PNGs remain separate,
unchanged and hash-verified; this is an additional publication-reference packet.

**Fixture-only, not production SSO, live publication or seven-role acceptance.**
No real keys, accepted API state, containers or production code changes. No full
build, Vitest or typecheck rerun: only transient API generation, Vite asset bundling
and the nine bounded browser cases. Own preview stopped, junctions/scratch removed,
shared cache lock unchanged. Sanitized published evidence contains no host paths,
credentials or raw browser authentication reports. Historical sections below retain
their original source/qualification scope.

## Single-Task History: 15 October 2026

The panel labels all generated delivery and PM metadata events, including queued,
reconciled, assignment reserved, lease renewed, run bound, result accepted,
proposal submitted, owner feedback and PM completion. EN/RU labels distinguish
PM execution completion from whole-task completion; events never grant owner
confirmation rights. Exhaustive generated event typing remains enforced.

The nine-event correction passes41 focused tests, full typecheck/build, focused
strict lint/format, generated OpenAPI check and Chromium effective-theme checks
on Base UI8e944aa. Earlier full385-test/lint/format/packed-consumer results retain
their unchanged-input scope; a new full394-test run is not claimed.

Routing OpenAPI compatibility remains open: the single-task v2 family deliberately
omits legacy package fields, while v1 still requires them. A shared optional/
nullable schema does not prove legacy client compatibility. No fake package
values, blanket exception or live seven-role acceptance follows from this UI fix.
The older slices below retain their original v1 behavior and qualification scope.

## Явный Opt-In Публикации

B-SDLC-05 добавляет в существующий consent panel отдельный unchecked выбор
опубликованной project routing policy. Это не editor: UI читает существующий
`GET /api/v1/projects/{project_id}/sdlc/routing-policy` по Central/project ACL
только для текущего task owner с серверным `can_confirm` в Clarification.
404 означает отсутствие policy; permission/error/loading не выбирают её автоматически.
Без выбора тело confirm по-прежнему не содержит `expected_routing_policy_version`.
С выбором передаётся только точная просмотренная версия вместе с прежними hash/key.

UI фиксирует выбранный version/hash отдельно от query head. Refresh новой policy
не заменяет выбор и не меняет operation key: выбранная старая версия остаётся
видимой, но routed confirmation заблокирован. Stale CAS сохраняет оба checkbox;
владелец должен явно пересмотреть выбор. Key scoped к credential/task requirement
pin и exact routing version/hash либо legacy omission. Unknown POST не ретраится
автоматически; пока fresh context readback не получен, cached permissions не
разрешают повторную запись. Новый credential/revision/hash не наследует согласие.

Названия семи ролей и labels локализованы; technical IDs, package/catalog refs и
hash доступны в явно раскрываемом details. Refs означают только preparation,
`verification:declared`, `native_ready:false`, `dispatch_allowed:false`.
Response guard использует generated aliases и проверяет project/instance/version,
pinned package commit/catalog3, exact role/profile/key mapping и отсутствие
duplicate agent/namespace/workflow. Проверка ссылки не является native attestation.
Backend остаётся authority для CAS, ACL, consent, immutable snapshot и assignment.
Policy editor, Fleet calls, admission, run, ACK/release и scheduler не добавлены.

Reservation backend и единственная pending migration 000034 в этом UI-срезе не
изменялись. [Routing owner contract](SDLC_ROUTING_V1.md) и
[prepared reservation boundary](SDLC_RESERVATION_V1.md) остаются обязательными.

### Scoped Проверки Opt-In

На Node 22.20.0 прошли 74 tests в `src/api/sdlc.test.ts` и
`src/features/sdlc/ui/SdlcPanel.test.tsx`: явный выбор/unchecked legacy, exact
version, owner-only GET, source loading/error/ACL, stale selection/CAS с сохранением
inputs, unknown POST с failed context readback и stable replay key, новые pins,
malformed policy/package/catalog/role refs и duplicate identities.
Scoped TypeScript с read-only Base pin 9408802, focused ESLint, Prettier и
штатный generated-client OpenAPI drift check прошли. Rust/OpenAPI DTO не менялись;
перегенерация frozen JSON вручную или новый Rust build не выполнялись.

Isolated Playwright проверил 375/1920/2560 px, unchecked initial choice,
expanded technical refs, exact opt-in POST и queued readback. Overflow отсутствует,
JS errors нет; 403 в ACL negative check ожидаем. Скриншоты просмотрены.
Все API requests перехвачены только в test runner; live state/secrets/runtime/DB
не используются. Vite/Chromium закрыты в finally, QA containers не запускались.
Local ignored evidence: `.local/b-sdlc05-browser-1791046550447/`.
Full build/CI/push/native admission не запускались и не заявляются.

Source slice B-SDLC-05, 2026-10-03, on `feat/pm-clarification` / existing PR114.
Source commit follows scoped checks only; no push, accepted runtime change or full
build. The integrated full gate remains separate. Backend intent invariants
and remaining admission gaps: [SDLC_LIFECYCLE_V1.md](SDLC_LIFECYCLE_V1.md).
Existing Base UI components/design are reused; no separate scheduler or operator
product actions are introduced. Private Base role/skill content is not copied.

## Implemented Wire

The existing issue-detail page exposes `?tab=sdlc`. The same authenticated API
client reads the strict SDLC owner resources; there is no Fleet call from this UI.

| Request | Actual authority / use |
| --- | --- |
| `GET /api/v1/issues/{id}/sdlc/context` | v1 instance/project/task/root/owner, current stage/revision/reason and server permissions |
| `GET /api/v1/issues/{id}/sdlc/requirements/{revision}` | Exact current immutable full document and SHA-256, never reconstructed from issue description |
| `POST /api/v1/issues/{id}/sdlc/requirements/{revision}/confirm` | Body `{content_hash, idempotency_key}` plus `expected_routing_policy_version` only after separate explicit publication selection; existing owner-session/ACL/evidence checks remain backend authoritative |
| `GET /api/v1/projects/{project_id}/sdlc/routing-policy` | Existing published seven-role declared policy; exact version for explicit publication, not native readiness |
| `GET /api/v1/issues/{id}/sdlc/analysis-intent` | Frozen intent readback only when context is Analysis; missing/inconsistent retained intent is not a successful queue state |
| `GET /api/v1/issues/{id}/sdlc/events?projection=metadata_v1&after=0&limit=100&max_bytes=65536` | Typed, bounded metadata history, decimal server cursors and explicit next/previous pages; not a durable consumer ACK |

Types come from the official generated API, not a second wire schema. Analysis
intent fields are `contract_version`, `tracker_instance_id`, `project_id`, `task_id`,
`root_task_id`, `intent_id`, `confirmation_id`, `requirement_revision`, `content_hash`,
`stage`, `status`, `role`, `workflow`, `mode`, `scope`, `cycle`, `attempt`,
`operation_key`, `created_at`. The adapter rejects inconsistent binding/revision/hash
and anything other than `Analysis / Ready / Analyst / hermes-sdlc:analyst /
analysis / business`, cycle 0 / attempt 0 and `analysis:<confirmation UUID>`.
Operation identity remains Tracker-issued and authoritative; the browser's
confirmation idempotency key is not an assignment operation key or fence.

The UI consumes all eleven generated metadata event variants, including
`analysis.intent_created` and `analysis.assignment_reserved`, as historical events. It displays actual event ref,
sequence, timestamp, metadata hash and Analysis intent/revision references.
It never interprets an event as assignment acceptance, a run or terminal success.

## State And Freshness

All ten requirements document fields are shown before consent. Confirmation is
available only from backend `can_confirm`, Clarification stage and a fresh successful
read. New credential/revision/hash resets the checkbox. Writes are not auto-retried;
an uncertain response can be explicitly retried with the same per-pin key while
this panel is mounted. Server replay/conflict rules remain the authority.

The historical `stage: Backlog` confirmation receipt is not an optimistic Analysis
transition. Only subsequent validated context/intent readback displays
`Analysis / Ready`, explicitly queued awaiting admission. Confirm completion
refetches the snapshot and invalidates metadata and existing issue/project/backlog
queries. There is no generated assignment, active run, admission success or Fleet
configuration pin, because those are not data supplied by this Tracker resource.

Displayed read times are local successful query receive times, independently for
snapshot and metadata, not server `observed_at` or a liveness guarantee. Intent and
event creation times come from the server. Failed refresh preserves the previously
read snapshot with an explicit stale/error state and disables consent; 401/403 hide
cached owner pins entirely. Credential changes get a fresh ephemeral cache namespace
without bearer values in cache keys. Query abort signals are forwarded.
404/no context, Draft/no document, no intent, empty metadata, loading, permission
denial, revision conflict and unavailable source remain distinct states. Metadata
errors are not rendered as an empty history. No production fixtures or mock-success
fallback exists. Restart reads retained server state rather than browser live state.

## Exact Source Files

| Group | Files relative to this checkout |
| --- | --- |
| Typed API + contract tests | `frontend/src/api/sdlc.ts`, `frontend/src/api/sdlc.test.ts`, test-only `frontend/src/api/test-fixtures/routing-policy.ts` |
| Query lifecycle | `frontend/src/features/sdlc/model/use-sdlc.ts` |
| Owner panel + component tests | `frontend/src/features/sdlc/ui/SdlcPanel.tsx`, `frontend/src/features/sdlc/ui/SdlcPanel.test.tsx` |
| Existing page integration + URL test | `frontend/src/pages/issue-detail/index.tsx`, `frontend/src/pages/issue-detail/issue-detail.test.tsx` |
| Existing localization | `frontend/src/shared/i18n/locales/en.json`, `frontend/src/shared/i18n/locales/ru.json` |
| Query annotation + schema contract test | `backend/api/src/routes/sdlc.rs`, `backend/api/src/lib.rs` (preserve B-SDLC-01 edits) |
| Official generated OpenAPI | `openapi/openapi.json` |
| Owner documentation | `docs/API.md`, `docs/ARCHITECTURE.md`, `docs/TZ.md`, `docs/SDLC_LIFECYCLE_V1.md`, this document |

`frontend/src/api/generated.ts` is regenerated by the existing `generate:api`
script and remains intentionally gitignored. No lockfile/dependency update, second
migration or assignment ledger change belongs to this UI slice; pending 000034 is
unchanged relative to the preceding B-SDLC-01 slice.

## Scoped Evidence Первого UI-Среза

PASS: 54 tests in the API adapter, owner panel and issue-detail test files;
exact consent/readback, stable retry key, stale revision reset, access denial,
credential change, missing credential, stale confirmation without automatic retry,
metadata refresh failure, empty/error states and bounded metadata cursors are covered.
The one focused Rust metadata schema test passed; the official offline Rust
`gen-openapi` exporter was rerun. SHA-256 of that B-SDLC-05 export (later backend
guard annotations may update the canonical schema):
`3a31df12b5b535caff904b83dda90fb0d82c26f6023590344c8c4efea64ec13e`.
Official generated-client byte consistency, scoped TypeScript, focused ESLint and
Prettier checks passed. No full workspace build/test/clippy or new DB test was run.

Frontend checks use Node 22.20.0 and the existing dependency cache, with read-only
public Base source at the actual `.base-revision` pin
`9408802dfa978cba2f67162a49adca6f65851b01` via ignored isolated QA configuration.
The ordinary installed `@pdlc/ui` cache is stale: an unmodified full frontend
typecheck reports three existing SSO export/signature errors in login files.
Those unrelated files and dependency resolution were not changed. Rematerializing
the pinned Base package belongs to the integrated source gate; this slice's scoped
typecheck against the actual pin is clean.

Isolated Playwright checked the production panel and API adapter with test-only
HTTP interception at 375, 1920 and 2560 px: exact POST, validated queued readback,
metadata event, no document/text overflow, and denied readback hiding pins.
Screenshots were visually inspected. Browser JS errors: zero; the intentional
403 is expected. Temporary Vite and Chromium were closed in `finally`; no API
request reached accepted/shared runtime or DB. These checks do not prove real
Fleet/Workflow admission or live autonomy. Local ignored evidence:
`.local/b-sdlc05-browser-1791027021266/` and
`.local/tracker-b-sdlc05-363ab1bf/openapi.json`.

## Next Owner Interface

Main reports SOURCE Fleet `GET /internal/runtime/v1/agents/{agent_id}/configuration`
with fresh bounded Base PAT introspection, exactly `[fleet-control:read]`, a fixed
dedicated `configuration_reader_subject` and deployment-owned concrete agent UUID
allowlist `configuration_reader_agent_ids`. The actual Base PAT issuer emits only
registered service read/write scopes; compound grants are not supported. This
readback grants no machine task delegation or run rights. It provides v1
`observation_ref`, agent/role/effective revision, pinned public package metadata,
`observed_at`, `managed_files_verified:true`, **`runtime_ready:false`**, native/workflow
blockers. This is the owner configuration observation source, not assignment ACK
or a permission to claim. The UI neither exposes the machine credential nor treats
that observation as installed/runtime-ready admission.

Tracker-only prepared reservation wire/capacity/heartbeat/readback теперь реализованы
в отдельном B-SDLC-01 срезе, но не являются Fleet acceptance/claim.
Machine native capabilities и trusted ACK/verified-stop остаются blockers.
Operation hash/key, capacity/fences/lease и outbox остаются Tracker authoritative;
expiry не освобождает live/unknown capacity. Явный UI opt-in эти gates не снимает.
