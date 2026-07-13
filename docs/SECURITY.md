# Security — Task Tracker

## 1. Overview

Task Tracker — self-hosted приложение с конфиденциальными данными проектов. Безопасность встроена на всех уровнях: transport, auth, storage, application, operations.

## 2. Authentication

- Passwords hashed with **argon2id**.
- JWT access token (15 min) + httpOnly refresh cookie (7 days, rotation).
- Failed login lockout после 5 попыток на 15 минут.
- MFA/TOTP — future, интерфейс заложен в `users.mfa_enabled`.
- OAuth/OpenID/LDAP — future, через admin settings.

## 3. Authorization

- Role-based access control (RBAC) per project.
- Issue-level security schemes (future).
- Permission checks на service layer, повторно — на repository layer.
- No data returned until permission verified.

## 4. Transport

- HTTPS/TLS everywhere в production.
- HSTS header.
- Secure, SameSite=Lax/Strict, httpOnly cookies.
- No sensitive data в URL query params.

## 5. Input Validation

- Strict DTO validation на входе (validator, zod).
- Whitelist mime-types для attachments.
- Filename sanitization.
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
  TASKTRACKER_CORS_ALLOWED_ORIGINS=https://tasktracker.example.com
  ```
- No wildcard (`*`) в production.

## 9. Secrets Management

- All secrets via env vars.
- No secrets in git.
- `.env.example` contains placeholders only.
- Rotate JWT/refresh secrets periodically.
- Database credentials separate from app config.

## 10. File Upload Security

- Size limits per type.
- Magic bytes validation.
- ClamAV virus scan.
- Quarantine bucket for positives.
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

- Non-root user в Docker images.
- Read-only filesystem где возможно.
- Distroless/alpine final images.
- No secrets в image layers.
- Scan images with Trivy.

## 15. Network

- PostgreSQL и Redis доступны только в internal network.
- Traefik на edge.
- Firewall: 19876, 80, 443 only.

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
- `docs/AUTH_ADVANCED.md` — детали refresh rotation, reuse detection, rate limits.
- `docs/SECURITY_INCIDENT_RESPONSE.md` — план реагирования на инциденты.

## References

- `docs/ARCHITECTURE.md`
- `docs/DEPLOYMENT.md`
- `docs/API.md`
