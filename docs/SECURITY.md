# Security — Task Tracker

## 1. Overview

Task Tracker — self-hosted приложение с конфиденциальными данными проектов. Безопасность встроена на всех уровнях: transport, auth, storage, application, operations.

## 2. Authentication

- В платформенном режиме Central Auth выполняет Authorization Code + PKCE,
  проверяет одноразовые `state`/`nonce` и владеет браузерной сессией. Task Tracker
  принимает только подписанный access token с ожидаемыми issuer/audience и
  проверяет активность центральной сессии.
- Локальный профиль связывается только по проверенному `sub`. Совпавший email
  исторического профиля не используется для автоматического связывания.
- При заданном `TT_AUTH__CENTRAL_JWKS_URI` локальные register/login/refresh,
  password reset и TOTP routes не монтируются. Недоступность Central Auth даёт
  явный `503`, а не fallback на локальный пароль.
- Исторический password/refresh/TOTP код и зашифрованные TOTP secrets сохраняются
  для совместимости данных, но второй фактор Task Tracker отключён. Пользователь
  видит предупреждение о снижении защиты на странице входа.
- В legacy-режиме без Central Auth пароли по-прежнему хешируются Argon2id, а
  локальные access/refresh, password reset и ранее реализованный TOTP работают по
  прежним контрактам.
- SAML/LDAP — не реализовано (future).

## 3. Authorization

- В центральном режиме любой активный вошедший пользователь имеет доступ ко
  всем проектам и пользовательским операциям. Исторические локальные роли и
  memberships сохраняются, но не ограничивают людей. Dashboard не показывает
  их как настройку доступа; исполнителей выбирают из активного центрального каталога.
- Личные API-токены проверяются Central Auth по audience, сроку, отзыву и scope
  `task-tracker:read` / `task-tracker:write`.
- Runner/service/runtime credentials остаются отдельной машинной границей и не
  получают пользовательские права автоматически.
- Legacy-режим сохраняет прежний project RBAC.

### SDLC: Strict Project And Assignment Boundary

SDLC routes do not inherit the ordinary central-mode project bypass above.
They recheck active central-subject identity and explicit project ownership or
membership. Business answers and exact-revision confirmation require the owner
human session; an operator cannot consent for that owner.

`GET /api/v1/sdlc/project-directory` uses that strict boundary, not ordinary
central-mode project listing. One SQL snapshot checks the active local identity
by verified central subject and explicit project ownership/membership before
UUID-keyset paging. Admin role, matching email/local user UUID, public/legacy
access, cookie-only credentials and unrelated service grants do not confer
directory access. Every page and replay uses fresh ACL, without a scope cache.
Only canonical project ID/key/name and a nullable continuation cursor are exposed;
there are no descriptions, counts or owner/member attributes. An authorized
nonmember (including global admin) receives an empty page, not the global catalog.
A cursor never proves continued access or authorizes creation/admission.

The PM ownership lease is machine-only, including readback and historical
replay. Its holder is derived from the verified bearer and must match the current
persisted assignment subject, exact PM grant and fence. Central service scopes
still apply. Fresh ACL/account checks cannot be bypassed by idempotency replay.
Expired ownership is not automatically reacquired; old receipts do not authorize
side effects or extend TTL. The receipt always has `dispatch_allowed=false` and
cannot replace future native/workspace/first-step admission checks. See
[the contract](CHAT_CLARIFICATION_CONTRACT.md#pm-execution-ownership-lease-not-admission).

## 4. Transport

- HTTPS/TLS everywhere в production.
- HSTS header.
- Secure, SameSite=Lax/Strict, httpOnly cookies.
- Токены не передаются в URL. SSE использует fetch-stream с `Authorization`.

## 5. Input Validation

- Strict DTO validation на входе (validator, zod).
- Whitelist заявленных content-type для attachments.
- Filename sanitization для path/control-символов.
- SQL только через parameterized queries / ORM.
- No `eval`, no dynamic SQL.

## 6. XSS / CSP

- CSP policy:
  ```
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: blob: {storage-origin};
  connect-src 'self' {api-origin};
  font-src 'self';
  object-src 'none';
  frame-ancestors 'none';
  base-uri 'self';
  form-action 'self';
  ```
- User-generated content escaped при render.
- Rich text — TipTap с whitelist nodes/marks.

## 7. CSRF

- SameSite cookies.
- Stateless CSRF token для mutation endpoints при необходимости.

## 8. CORS

- Strict whitelist:
  ```
  TASKTRACKER_SERVER__CORS_ALLOWED_ORIGINS=https://tasktracker.example.com
  ```
- No wildcard (`*`) в production: wildcard CORS does not allow credentialed refresh-cookie requests.
- Credentials только при trusted origin.

## 9. Secrets Management

- All secrets via env vars.
- No secrets in git.
- `.env.example` contains placeholders only.
- Rotate JWT/refresh secrets periodically.
- Database credentials separate from app config.

## 10. File Upload Security

- Size limits per type.
- Whitelist заявленных content-type для attachments.
- Filename sanitization для path/control-символов.
- Magic bytes validation — не реализовано (future).
- ClamAV virus scan — не реализовано (future).
- Quarantine bucket — не реализовано (future).
- No direct execution of uploaded files.

## 11. Rate Limiting

- `tower_governor` per IP and per user.
- Stricter limits for auth endpoints.
- WebSocket connection limits per user.

| Endpoint | Limit |
|----------|-------|
| Login | 5/min |
| Register | 3/min |
| API general | 100/min |
| Search/JQL | 60/min |

## 12. Audit Logging

- Login/logout events.
- Permission changes.
- Project/role modifications.
- Admin actions.
- Stored in `audit_log` table, retained 1 year.

## 13. Dependency Security

- `cargo audit` в CI.
- `pnpm audit` в CI.
- Dependabot/Renovate alerts.
- Pin major versions.

## 14. Container Security

- Backend работает под non-root пользователем `tasktracker` (uid 999); volume `uploads` нормализуется one-shot сервисом `uploads-init`.
- Обязательные секреты: `POSTGRES_PASSWORD`, `TASKTRACKER_JWT_SECRET` (`${VAR:?}` в compose — без них стек не стартует).
- Read-only filesystem / distroless images — не реализовано.
- Image scan (Trivy) — не реализовано.

## 15. Network

- PostgreSQL и Redis не публикуются наружу (internal compose-сеть; порты `ports:` отсутствуют).
- Наружу открыты только `frontend` (19877) и `backend` (3456); значения меняются `FRONTEND_PORT`/`BACKEND_PORT`.
- Traefik — опциональный profile (`--profile traefik`).
- Firewall-правила хоста настраиваются администратором.

## 16. Incident Response

- Rotate compromised secrets.
- Revoke sessions via admin panel.
- Block users.
- Export audit log.

## 17. Security Headers

```
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: geolocation=(), microphone=(), camera=()
Content-Security-Policy: ...
```

## 18. Penetration Testing

- Internal security review перед релизом.
- OWASP ZAP scan в CI.
- Bug bounty — future.

## 19. Data Privacy

- No personal data in logs.
- GDPR/CCPA delete account endpoint (future).
- Data retention policies.

## 20. References

- `docs/API.md` — auth flow.
- `docs/SYSTEM_ADMIN.md` — users/groups/permissions.
- `docs/STORAGE.md` — attachment security.
- `docs/ERROR_HANDLING.md` — error disclosure.
- `docs/SECURITY.md` — детали refresh rotation, reuse detection, rate limits.
- `docs/SECURITY.md` — план реагирования на инциденты.

## References

- `docs/ARCHITECTURE.md`
- `docs/DEPLOYMENT.md`
- `docs/API.md`
## SDLC Clarification Authorization

PM credential confinement rejects any PM assignment grant on the legacy
router before Central context becomes UserClaims. Strict SDLC requests with that
grant require one canonical task capability, allowed method/path and current
assignment subject/scope plus matching persisted assignment ledger on reads.
Owner answers/confirmation, verifier evidence and assignment/binding writes are
not PM operations. Generic human/service PAT behavior is unchanged. These checks
passed actual TCP/PostgreSQL regressions, including latest-ledger validation on
lease reads/claim/heartbeat. The synthetic Central fixture is not real Base
delegation or runtime handoff; that acceptance remains required before admission.

SDLC uses a dedicated central-only middleware, live session/PAT checks and
service scopes. PostgreSQL rechecks active central subject and explicit project
owner/member rows under locks on every read/write/replay. Legacy global admin
and TT_AUTH project bypass confer no SDLC access. Answer/confirm require the
exact persisted owner and a human session; PM writes require current assignment,
execution, concrete agent/version and exact scope. A separate configured verifier
attests readiness; owner/PM boolean claims cannot pass checks. See the
[contract](CHAT_CLARIFICATION_CONTRACT.md) for provisioning and fail-closed errors.
