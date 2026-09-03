# Task Tracker — General Review Report (ARCHIVED)

> **⚠️ This document is a snapshot from 2026-08-26 and is preserved for
> historical reference only.** Many findings have been resolved in subsequent
> commits. For the current audit status, see `review-findings.md` at the
> repository root.

**Date:** 2026-08-26
**Scope:** Full project audit — backend, frontend, docs
**Method:** 3 parallel deep-audit subagents + manual verification

---

## P1 — Critical (must fix)

### Security: Missing authorization (IDOR)
| # | File | Issue |
|---|------|-------|
| 1 | `api/src/routes/issues.rs:238` | `delete_issue` — no UserClaims, any user can delete any issue |
| 2 | `api/src/routes/issues.rs:255` | `restore_issue` — no UserClaims, any user can restore any trashed issue |
| 3 | `api/src/routes/issues.rs:272` | `purge_issue` — no UserClaims, any user can permanently delete any issue |
| 4 | `api/src/routes/members.rs:58` | `add_member` — no UserClaims, any user can add members to any project |
| 5 | `api/src/routes/members.rs:102` | `remove_member` — _claims unused, any user can remove any member |
| 6 | `app/src/services.rs:1496,1526,1537` | Label create/update/delete — `_requester` ignored, no project auth |
| 7 | `app/src/services.rs:1446` | Attachment delete — `_requester` ignored |
| 8 | `app/src/services.rs:1648` | IssueLink delete — `_requester` ignored |
| 9 | `app/src/services.rs:2176,2215` | CustomField create/update — `_requester` ignored |

### Frontend: Silent data loss
| # | File | Issue |
|---|------|-------|
| 10 | `api/worklog.ts:22` | `remainingEstimate` collected in LogWorkDialog but never sent to API — user input silently discarded |

### Docs: Stale/misleading
| # | File | Issue |
|---|------|-------|
| 11 | `docs/API.md:1285` | WebSocket section — backend uses SSE, not WebSocket |
| 12 | `docs/ROUTING.md` | 39 routes documented but not in router.tsx (13 actual routes) |
| 13 | `docs/DATA_MODEL.md` | 12 of 26 actual migration tables missing from "actual schema" section |

---

## P2 — Major (should fix)

### Backend: Missing integration tests (30 endpoints)
**Endpoints with 0 integration tests:**
- soft-delete: `DELETE /issues/{id}`, `POST /issues/{id}/restore`, `DELETE /issues/{id}/trash`, `GET /projects/{key}/trash`
- auth: `POST /auth/refresh`, `POST /auth/logout`, `GET /auth/me`, `GET /users`
- project: `PATCH /projects/{key}`, `DELETE /projects/{key}`
- watchers/votes: `POST/DELETE/GET /issues/{id}/watch`, `POST/DELETE/GET /issues/{id}/votes` (6 endpoints)
- components: `GET/POST/PUT/DELETE` (4 endpoints)
- versions: `GET/POST/PUT/DELETE` (4 endpoints)
- custom fields: `GET/POST/PUT/DELETE /custom-fields`, `GET/PUT /issues/{id}/custom-fields` (6 endpoints)

### Frontend: Accessibility (15 icon buttons without aria-label)
- `issue-detail` (2), `project-backlog` (2), `project-board` (2), `project-trash` (1), `projects` (5), `dialog.tsx` (1), `app-shell.tsx` (1), `IssueDescriptionEditor` (1)

### Frontend: Hardcoded strings (11)
- `issue-create`: priority options (Medium/Highest/High/Low/Lowest)
- `issue-detail`: "Custom fields", "≡ TaskTracker"
- `app-shell.tsx`: "TaskTracker", "Toggle menu"
- `LogWorkDialog`: "What did you do?"
- `ProjectFormDialog`: "TT" placeholder

### Frontend: Missing feature tests (12 components)
- ActivityFeed, AttachmentPanel, CustomFieldsPanel, IssueDescriptionEditor, IssueMetaEditor, LabelEditor, LinkEditor, ProjectMembersPanel, ProjectFormDialog, SprintFormDialog, TimeTrackingPanel, WorklogTab

### Frontend: Dead code (13 unused exports)
- `api/admin.ts`: AuditLogEntry, `api/issue-create.ts`: CreateIssueInput, `api/issue.ts`: UpdateIssueInput, `api/members.ts`: AddMemberInput, `api/notifications.ts`: NotificationFrequency, `api/worklog.ts`: toHuman, `entities/worklog/model.ts`: CreateWorklogPayload, `features/comments/ui/CommentList.tsx`: CommentForm + CommentItem, `shared/api/hooks.ts`: 9 exported query-key objects, `shared/lib/time.ts`: formatDurationShort, `shared/ui/button.tsx`: ButtonProps, `shared/ui/textarea.tsx`: TextareaProps

### Docs: Accuracy (20+ documented endpoints don't exist)
- API.md documents: boards CRUD, workflows CRUD, schemes (20+), issue-link-types, issue-types CRUD, standalone sprints, user management, password reset, issue assign/clone/move, import/export, project settings, worklogs reports — **none exist in OpenAPI spec**
- API.md says "42 paths" but spec has 70 paths / 99 endpoints
- Port 19876 in API.md/CLI.md/DEPLOYMENT.md — doesn't match docker-compose (backend 3456, frontend 19877)

### Docs: Stale references
- DATA_MODEL.md: saved_filters + dashboards tables (removed/unimplemented)
- ROADMAP.md: says features "postponed" but they're implemented in v0.2.0
- CLI.md: says "commands are stubs" but CLI has 12 fully implemented groups
- AGENTS.md: claims "shaku DI" but shaku not in any Cargo.toml
- CONTRIBUTING.md: references .husky but project uses lefthook

---

## P3 — Minor (nice to fix)

### Backend
- `list_trash` — no project membership check
- issue_type/priority silently default on invalid input (should 400)
- `.expect()` in metric_handle() — startup panic risk
- JQL useEffect not debounced (URL churn)

### Frontend
- 30 OpenAPI endpoints have no frontend hook (votes, watchers, transition, sprint management, components, versions, etc.)
- issue-detail uses raw useQuery instead of hooks.ts pattern
- Error handling inconsistent: `text-rose-500` + raw `error.message` (6 pages) vs `text-danger` + `t()` (admin)
- notifications.ts uses JSON.stringify(error), others use static strings
- search page: JQL useEffect not debounced

### Docs
- Missing referenced docs: JQL.md, WEBSOCKET_EVENTS.md, AUTH_ADVANCED.md
- DOMAIN_MODEL.md references removed SavedFilter/Dashboard entities
- No root AGENTS.md (only docs/AGENTS.md)
- No CHANGELOG.md (referenced by CONTRIBUTING.md)
- No CONFIG.md
- CORS config undocumented

---

## Summary by category

| Category | P1 | P2 | P3 | Total |
|----------|----|----|----|-------|
| Security (IDOR) | 9 | — | 1 | 10 |
| Frontend bugs | 1 | — | 4 | 5 |
| Missing tests | — | 42 | — | 42 |
| Accessibility | — | 15 | — | 15 |
| Hardcoded strings | — | 11 | — | 11 |
| Dead code | — | 13 | — | 13 |
| Docs accuracy | 3 | 20+ | 8 | 31+ |
| **Total** | **13** | **101+** | **13** | **127+** |

## References

- [ARCHITECTURE](ARCHITECTURE.md)
- [LOCAL_SETUP](LOCAL_SETUP.md)
