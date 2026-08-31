# Security — Task Tracker

## 1. Overview

Task Tracker — self-hosted приложение с конфиденциальными данными проектов. Безопасность встроена на всех уровнях: transport, auth, storage, application, operations.

## 2. Authentication

- Passwords hashed with **argon2id**.
- JWT access token (15 min) + httpOnly refresh cookie (7 days, rotation).
- Failed login lockout после 5 попыток на 15 минут.
- MFA/TOTP — не реализовано (future).
- OAuth/OpenID/LDAP — не реализовано (future).

## 3. Authorization

- Role-based access control (RBAC) per project.
- Issue-level security schemes (future).
- Permission checks на service layer, повторно — на repository layer.
- No data returned until permission verified.

## 4. Transport

- HTTPS/TLS everywhere в production.
- HSTS header.
- Secure, SameSite=Lax/Strict, httpOnly cookies.
- No sensitive data в URL query params, кроме короткоживущего `access_token` fallback только для `GET /api/v1/events`, где browser `EventSource` не позволяет задать `Authorization`.

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
