# Task Tracker — Full Audit Report (2026-09-03)

Commit: `722e074` (main, clean). Baseline verified:

- Backend: `cargo test --workspace` — **487 passed / 0 failed / 19 ignored**; all 19 ignored (14 repos + 3 FK + 2 server e2e) **pass** with `scripts/run-e2e-tests.sh` against real PostgreSQL. `cargo clippy -D warnings` — clean.
- Frontend: vitest — **131 passed / 41 files**; `tsc --noEmit` — 0 errors; `eslint --max-warnings=0` — clean.
- CI (`.github/workflows/ci.yml`): fmt+clippy+test, OpenAPI sync check, migrations on clean PG, coverage ≥60%, cargo-deny, real-PG infra-db-tests job, frontend job.
- Docs: 0 broken local links; versions 0.2.0 consistent (Cargo/package.json/openapi/CHANGELOG); OpenAPI 70 paths == router registrations.
- Security posture: centralized `Authz` policy layer (IDOR closed across all scoped routes), error.rs no longer leaks internals, Argon2id, refresh rotation with advisory-lock CAS, WIP-limit atomic, attachments MIME allowlist + path-traversal protection, CSP/nosniff/DENY headers, rate limits (auth + general).

Verified findings below. Rejected during verification (false positives): run-e2e-tests.sh service-name mismatch (compose normalizes `postgres_test`→`postgres-test`); "PATCH status_id bypasses workflow" (backend validates identical rules in update path); "PG tests effectively disabled" (CI infra-db-tests + local script both run them).

---



## Fix Status (PR #48 + this commit)

| Finding | Status |
|---------|--------|
| P1.1–P1.2 scripts | ✅ Fixed (PR #48) |
| P1.3 allow_registration | ✅ Fixed (PR #48) |
| P1.4 e2e smoke mock | ✅ Fixed (PR #48) |
| P1.5 refresh profile | ✅ Fixed (PR #48) |
| P2.1 API.md comments | ✅ Fixed (PR #48) |
| P2.2 CHANGELOG env/CLI | ✅ Fixed (PR #48) |
| P2.3 CONTRIBUTING hooks | ✅ Fixed (PR #48) |
| P2.4 REVIEW.md archived | ✅ Fixed (PR #48) |
| P2.5 backup/restore scripts | ✅ Fixed (PR #48) |
| P2.6 password policy | ✅ Fixed (PR #48) |
| P2.7 N+1 users/projects | ✅ Fixed (PR #48) |
| P2.8 SSE per-event SQL | 📋 Deferred (requires moka cache integration) |
| P2.9 N+1 sprints/watchers/votes | ✅ Fixed (PR #48 — votes count; sprints/watchers batched) |
| P2.10 dead infra code | ✅ Fixed (PR #48) |
| P2.11 dead UI buttons | ✅ Fixed (PR #48) |
| P2.12 i18n interpolation/42 | ✅ Fixed (PR #48 + this commit — placeholders added, 42→backlog_total) |
| P2.13 OpenAPI IssueResponse timestamps | ✅ Fixed (PR #48) |
| P3.1 stale doc details | ✅ Fixed (PR #48) |
| P3.2 votes count from page | ✅ Fixed (PR #48) |
| P3.3 updated_at overwritten | ✅ Fixed (this commit — issue_active_model uses domain value) |
| P3.4 logout race | ✅ Fixed (PR #48 — atomic clear_refresh_token) |
| P3.5 next_issue_number O(n) | ✅ Fixed (PR #48 — SQL MAX) |
| P3.6 O(n²) accessible_project_ids | ✅ Fixed (this commit — HashSet) |
| P3.7 naive date boundary | 📋 Deferred |
| P3.8 FE minor (a→Link, empty dirs, avatar memo, parseDuration) | ✅ Fixed (PR #48 + this commit — avatar memo) |
| P3.9 test-quality gaps | ✅ Partial (this commit — 500-tests assert exact status; coverage gate unchanged) |
| P3.10 worklog remainingEstimate | ✅ Fixed (this commit — dead field removed from FE model) |
| P3.11 ProjectKey validation | ✅ Fixed (this commit — is_valid() in labels/components_versions) |
| P3.12 Traefik labels | 📋 Deferred (infra config, not code) |
| D1 CACHING.md/ARCHITECTURE.md stale refs | ✅ Fixed (this commit) |
| D2 BoardResponse backlog_total | ✅ Fixed (this commit) |
| D3 i18n board.subtitle placeholders | ✅ Fixed (this commit) |
| D4 worklog remainingEstimate | ✅ Fixed (this commit) |

---

## P1 — Broken functionality / guaranteed-broken paths

| # | Finding | Evidence |
|---|---------|----------|
| P1.1 | Deploy scripts reference non-existent files/services: production/staging deploy guaranteed to fail | `scripts/deploy-production.sh:10-12`, `scripts/deploy-staging.sh:10-12` use `docker-compose.prod.yml`/`.staging.yml` (absent) and `migrator` compose service (absent in `docker-compose.yml`) |
| P1.2 | Init/admin scripts call missing compose services and non-existent CLI commands | `scripts/init.sh:39` (`migrator` service missing); `scripts/init-admin.sh:28` (`cli` service missing + CLI has no `users create-admin` — file even says "Placeholder"); `scripts/reset-admin-password.sh:27`, `scripts/seed-demo.sh:20` (same) |
| P1.3 | `security.allow_registration` admin setting is dead — register endpoint never checks it | Setting allowlisted in `app/src/services/admin.rs:31`, but `AuthService::register` (`app/src/auth.rs:24-47`) has no check → closed-instance toggle does nothing |
| P1.4 | E2E smoke mocks notifications with wrong contract key | `frontend/e2e/smoke.spec.ts:233,245` mock `{items: []}`; real `NotificationListResponse` key is `notifications` (`src/api/generated.ts:1422`) → smoke validates a shape the API never returns |
| P1.5 | Auth refresh drops user profile fields | `src/api/client.ts:27-48` parses only `access_token/user_id/email`, ignores rest of `AuthResponse` (username/displayName); register form never sends `name` (`src/pages/register/index.tsx:29-31`) → after page-reload refresh profile degrades to email |

## P2 — Incorrect behavior / docs-vs-reality / performance

| # | Finding | Evidence |
|---|---------|----------|
| P2.1 | API.md documents non-existent comment endpoints | `docs/API.md:710-712` — `PUT/DELETE /issues/{id}/comments/{commentId}`; actual: `PATCH|DELETE /api/v1/comments/{id}` (`backend/api/src/lib.rs:529-532`) |
| P2.2 | CHANGELOG claims wrong env format and phantom CLI groups | `CHANGELOG.md:45` — `TASKTRACKER__SECTION__KEY` (real: `TASKTRACKER_SECTION__KEY`, `config.rs:186-188`); `CHANGELOG.md:11-14` claims CLI groups watchers/votes/custom-fields — absent in `backend/cli/src/main.rs` |
| P2.3 | CONTRIBUTING describes hooks that don't exist | `CONTRIBUTING.md:101-120` — lefthook `pnpm lint-staged` + lint-staged config not in repo; real hooks in `lefthook.yml` differ |
| P2.4 | REVIEW.md is a stale snapshot posing as current status | `docs/REVIEW.md:66,96` references missing `JQL.md`, `WEBSOCKET_EVENTS.md`, `AUTH_ADVANCED.md`, `CONFIG.md`; "42 paths" contradicts actual 70 |
| P2.5 | Backup/restore scripts: documented arg ignored, env vars mismatched, volume name hardcoded | `scripts/backup.sh` ignores argv (docs `DEPLOYMENT.md:83`, `BACKUP_RESTORE.md:21` pass one); `TASKTRACKER_DB_USER/NAME` (`backup.sh:17-18`, `restore.sh:16-17`, `init.sh:36`) absent from `.env.example` (which has `POSTGRES_USER/DB`); volume `task-tracker_uploads` hardcoded (`backup.sh:37`, `restore.sh:50`) — breaks with `-p`/renamed project |
| P2.6 | No password policy anywhere | `hash_password` (`app/src/auth.rs:164+`) accepts 1-char passwords on register and admin create |
| P2.7 | Every issue create/update/transition loads ALL projects + ALL users | `app/src/services/helpers.rs:267-292` (`build_issue_dtos_prefetched` → `projects.list(default)` + `users.list()`) |
| P2.8 | SSE per-event authorization = 2–3 SQL queries per event, uncached | `api/src/routes/events.rs:56-57,89-114` (`get_by_key` + `require_project_access` each event) |
| P2.9 | N+1 query clusters | `app/src/services/project.rs:48-68` (COUNT per status); `sprint.rs:134-139` (query per sprint); `watcher.rs:74-84`, `vote.rs:94-99` (get_by_id per watcher/vote) |
| P2.10 | Dead infra code: unused EventBus + cache | `infra/src/event_bus.rs` (own `DomainEvent`), `infra/src/cache.rs` (`AppCache`), `domain/src/events.rs` — none imported anywhere |
| P2.11 | Dead UI buttons on issue detail | `src/pages/issue-detail/index.tsx:121-127` — Edit/Comment `<Button>` without onClick |
| P2.12 | i18n: interpolation params silently ignored + hardcoded 42 | `src/pages/project-board/index.tsx:141-144` passes `{projectName,sprintName,backlog:42,remainingDays}` but `ru/en.json` board.title="Доска"/subtitle have no placeholders; `common.error` key missing (renders raw key) |
| P2.13 | OpenAPI IssueResponse omits created_at/updated_at that backend sends | `openapi/openapi.json` IssueResponse vs `IssueDto` (`app/src/dto.rs`) — generated client types incomplete |

## P3 — Minor / hardening / cleanup

| # | Finding | Evidence |
|---|---------|----------|
| P3.1 | Stale doc details: port 19876, CORS default, shaku, CI deny job undocumented | `docs/TZ.md:185`, `docs/LIBRARIES.md:256`, `docs/ARCHITECTURE.md:206`, `docs/CODE_REVIEW.md:42`, `docs/CI_CD.md:13,33` |
| P3.2 | Votes list `count` = page size, not repo count | `api/src/routes/watchers_votes.rs:237` (`votes.len()`), `count_votes` exists unused |
| P3.3 | `issue_active_model` always overwrites `updated_at=now()` | `infra/src/repos.rs:103` — ignores domain value; second `now()` after `change_status_atomic` skews vs status history |
| P3.4 | logout is read-modify-write, races with CAS refresh rotation | `app/src/auth.rs:102-106` vs advisory-lock `rotate_refresh_token` (`infra/src/repos.rs:197+`) |
| P3.5 | `next_issue_number` loads all project issue keys into memory | `infra/src/repos.rs:449-467` — O(n) strings per create; correctness handled by retry loop |
| P3.6 | Misc inefficiencies | `list_active_users` loads all users (`app/src/auth.rs:113-120`); `accessible_project_ids` O(n²) `Vec::contains` (`app/src/authz.rs:80-98`) |
| P3.7 | Digest day boundary uses naive date | `server/src/lib.rs:15-27` (`date_naive()`) |
| P3.8 | FE minor: `<a href>` instead of `<Link>` (login/register); empty dirs `src/i18n/lang/`, `src/services/`; UserAvatar regenerates SVG every render (no memo); Progress lacks `role="progressbar"`; `parseDuration` rejects `1.5m`; e2e hardcoded URLs/ports (`backlog-pagination-live.spec.ts:9`, `frontend-p1-fixes-live.spec.ts:4`) | respective files |
| P3.9 | Test-quality gaps | 500-path tests assert only `is_server_error()` (`api/tests/failing_repos.rs:134+`); `sleep(1ms)`+timestamp assertions flaky-prone (`app/src/services/tests.rs:5641,5660`); email tests happy-path only; migration crate has 0 own tests (CI compensates); coverage gate only 60% |
| P3.10 | Worklog entity field `remainingEstimateSeconds` never mapped from API (always undefined; UI falls back to estimate−spent) | `src/api/worklog.ts:8-26`, `src/entities/worklog/model.ts:7` |
| P3.11 | `ProjectKey::new` without `is_valid()` check in some routes (labels, components) | `api/src/routes/labels.rs:66`, `components_versions.rs:90,109` |
| P3.12 | Traefik compose service has no router labels (non-functional) + `--api.insecure=true` (dashboard loopback-only, acceptable) | `docker-compose.yml:102-115` |
