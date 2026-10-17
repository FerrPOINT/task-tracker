# Critical PM Lifecycle Persistence Companion

Source-only companion based on `e31f697cca9c8083340ae7709a77e58d4f573439`.
No shared hook, module, API or migration registration is applied by this packet.
It does not inherit earlier PG qualification and is not a live-ready claim.

## Storage And Atomic Boundary

- `sdlc_critical_pm_lifecycle_operations`: append-only exact before/after state,
  execution and blocker, actual lease/owner version, original request hash/body,
  result and deferred outbox/completion references. Existing command, claim,
  resume and completion original-key ledgers remain the public replay authority.
- `sdlc_task_routing_generations`: append-only generations2..MAX_SAFE_VERSION,
  exact previous snapshot, current review/completion/journal/owner confirmation.
  Payload copies the entire frozen policy, never current project policy.
- `sdlc_routing_snapshot_inventory`: read-only UNION ALL of original generation1
  and validated later generations. Columns, in order: `snapshot_id`, `task_id`,
  `project_id`, `policy_version`, `confirmation_id`, `requirement_revision`,
  `content_hash`, `payload`, `generation`. Both source-table INSERT directions
  take the same UUID advisory lock and reject cross-table snapshot collisions.
  Generation1 source rows and its UNIQUE(task_id) are unchanged; no backfill.
- Existing `sdlc_analysis_intents.routing_snapshot_id` retains its original FK.
  New `review_routing_snapshot_id` references the generations table through exact
  composite task/generation and task/confirmation/revision/hash FKs. New
  `routing_generation` is2..MAX_SAFE_VERSION on this branch. Both branch columns
  cannot coexist; missing generation/null partial combinations fail closed.
  Historical unrouted legacy rows remain on the old branch; the existing routing
  trigger still requires original FK when an original routed snapshot exists.

Claim and resume journal the actual existing lease, not a native Running receipt.
The canonical runtime is Reserved. Owner confirmation persists its actual
receipt but leaves Draft/Ready/critical_pm_awaiting_finalization, with NO new
Analysis intent. Its business event also says Draft, not a dispatch stage.

`finalize_critical_pm_review` consumes the server-prepared completion from the
SAME existing CompletePmDraft transaction. Order: completion journal, actual
completion and original operation, generation, Analysis intent, canonical
projection/admission Completed, pm.completed and analysis.intent_created outbox.
Deferred checks require every effect. A review completion inserted separately
cannot commit. The original failed/replayed public command is never rewritten.
No new capacity hold, queue, native proof, owner consent or runtime service exists.
Actual native/Workflow verification remains the existing trusted Fleet boundary.

## Exact Transaction Hooks

`critical-pm-lifecycle-hooks.patch` passes `git apply --check` against the stated
baseline but is deliberately NOT applied. Main owns application/registration.

All helpers borrow the caller's existing task/project-locked DatabaseTransaction
and never commit. `Ok(None)` means NO CURRENT REVIEW. A found review with invalid
cursor, owner, baseline, state, lease or history returns Conflict; never fall back
to the initial PM implementation. Historical resolved reviews are not active.

| Existing insertion point | Helper / outcome |
| --- | --- |
| `sdlc_native_admission.rs::execute_with_native`, after original replay and existing `verify_native_command`, before generic `apply` | `execute_critical_pm_review_command(tx,state,actor,command,hash) -> Result<Option<Json>>`; Some commits/returns instead of generic apply/finish |
| `sdlc_execution_lease.rs::claim_pm_lease`, after actual lease INSERT/original operation and load, before old status-only tail | `project_critical_pm_claim(tx,state,actor,command,hash,receipt) -> Result<Option<()>>`; Some commits/returns |
| `sdlc_pm_resume.rs::resume_pm_lease`, before generic validate_context | `allow_critical_pm_retained_head(tx,state,command) -> Result<Option<()>>`; Some admits only saved current-review Draft/approved baseline/head, including real latest confirmation |
| Same resume, after actual input receipt verification, dedup, original resume journal and lease CAS, before status-only tail | `project_critical_pm_resume(tx,state,actor,command,hash,receipt) -> Result<Option<()>>`; Some commits/returns; existing input/run/checkpoint uniqueness and current version CAS stay mandatory |
| `sdlc_reservation.rs::complete_pm_draft_execution`, AFTER preparing current receipt but BEFORE generic completion INSERT | `finalize_critical_pm_review(tx,state,actor,command,hash,prepared) -> Result<Option<()>>`; Some commits/returns; no prior committed completion prerequisite |

The companion preserves existing command/key/hash policies. Resume retains the
original body/hash in the old ledger; only a clone avoids consuming the hash
before the new helper. It never forges a scheduler principal or lease holder.

## Required Main-Owned Selectors

These are REQUIRED integration changes, not optional later phases. The shared
selectors and Peirce scoped readers are not edited in this packet.

1. `sdlc_lifecycle.rs::lifecycle_read` (and task/context reads validating current
   lifecycle): after existing root/project lock and parsed current receipt, call
   `self.review_snapshot(tx,state,receipt) -> Result<Option<TaskRoutingSnapshot>>`.
   Only Some permits its exact original-approved baseline during current PM edits;
   None must call unchanged `exact_revision`. No catch/conflict fallback. Do NOT
   put an unconditional relaxation in `exact_snapshot` or alter the raw `current`
   parser. The helper verifies current review/head/reservation/cursor, canonical
   task, immutable inventory/generation, original policy and actual owner consent,
   all inside that transaction. It requires no initial fake requirements/lease.
2. `sdlc_routing.rs::read_task_routing`: read inventory, select the actual active
   lifecycle routing UUID; never ORDER BY latest project policy. If no lifecycle,
   generation1 only. Later generation must match atomically finalized lifecycle.
3. `sdlc.rs::current_analysis_intent`: join inventory on
   `COALESCE(i.routing_snapshot_id,i.review_routing_snapshot_id)` AND same task;
   retain exact current owner confirmation/revision/hash/workflow comparison.
   A missing routed payload must conflict, not silently use the legacy default.
4. `sdlc_candidates.rs::CANDIDATES_SQL`: inventory plus exact active snapshot /
   latest confirmation; no duplicate task rows from UNION history. Initial seed
   eligibility remains generation1/current PM actually completed. Later Ready
   eligibility comes from the atomically finalized head, no second planner.
5. `sdlc_lifecycle.rs::delivery_seed`: no-head seed remains generation1 only.
   Later generations already have the finalized canonical head. Existing
   legacy Analysis reservation FK/gate remains original-table-only: generation2
   must use the real single-task lifecycle reserve, NOT a silently relaxed legacy
   Analysis reservation. `exact_revision` remains strict for technical writes.
6. `sdlc_critical_blockers.rs::ensure_progress_allowed` and blocker detail/current
   PM readers: project `sdlc_critical_pm_detail(h.blocker_id)`, including Resolved,
   rather than old COALESCE(return,held) or review-only fallback. Fresh progress
   still denies Held/AwaitingPmReview/InPmReview. Scoped owner/current grant and
   lease/auth readers remain authoritative, not this SQL projection function.
7. `sdlc_critical_blockers.rs::exact_history`: exact immutable inventory lookup
   by task AND archived assignment routing UUID AND full payload. UNION views
   cannot use FOR SHARE; existing task lock plus immutable source history apply.
   `original_producer` must read its exact saved Return by task/blocker WITHOUT
   requiring it to remain the current head, preserving old original-key replay.
8. `submit_critical_blocker_with_storage`: query actual generation by the exact
   current routing UUID AND payload; use it in BOTH existing BlockerAllocations
   instead of the literal1. Under the SAME root lock replace plain head INSERT:

```sql
INSERT INTO sdlc_critical_blocker_heads(task_id,blocker_id) VALUES($1,$2)
ON CONFLICT(task_id) DO UPDATE SET blocker_id=EXCLUDED.blocker_id
WHERE sdlc_critical_pm_detail(sdlc_critical_blocker_heads.blocker_id)->>'state'='resolved'
```

Require exactly one affected row or Conflict. The fragment's strict CAS trigger
also binds the new proposal to the actual current technical head and exact
resolved generation. Old held/review/return/journal rows remain immutable. This
supports subsequent bounded review cycles without reusing completed PM execution.

Main's Fleet header/current-routing builders must carry this exact selected
snapshot, not generation1/current-project-policy fallback. No API transport or
public command change is needed here.

## Qualification Request

Performed here: rustfmt parsing/formatting and companion patch applicability;
source contract checks only. No Cargo/Clippy/PG/Docker execution is claimed.

Four authored infra source-contract units use prefix
`sdlc::critical_pm_lifecycle::tests::` (run after module registration).
The NEW fixture child reuses the existing fresh DB fixture. Exact PG selector:

```text
sdlc::candidates::tests::critical_blocker_tests::pm_lifecycle_tests::actual_critical_pm_lifecycle_is_atomic_and_preserves_initial_history
```

The `candidates::tests` alias is verified in the baseline source. Collect nonzero
after applying the companion module hook before invoking `--exact`. Env is the EXISTING
`TT_CANDIDATES_TEST_DATABASE_URL`, fresh disposable DB name ending
`_candidates_test`. No new runner; every independent fixture needs its own fresh
DB. Existing test is ignored deliberately, like its neighboring PG fixtures;
owner executes the exact selector with `--ignored --exact --nocapture`.

The authored test covers actual claim, text question/owner answer, typed answer
resume/replay, new revision, narrow baseline read vs unchanged legacy rejection,
stale review conflict, exact consent, confirmation resume while remaining Draft,
absence of premature intent/completion, deferred packet rollback, stale lease,
one-winner completion/replay, exact generation/FK/policy/history and immutable
generation; native/Workflow evidence is explicitly synthetic.

Before acceptance main additionally needs actual populated0033 -> latest096
upgrade, whole existing legacy/hold/reservation regression selectors, cross-table
UUID collision (both insert directions/concurrent), partial/null/mixed intent
branches, missing completion/outbox/journal effects, wrong previous chain and
two complete technical->PM->Analysis review cycles after its selectors/head CAS
are wired. This packet does not claim those PG checks or completed full wiring.
