# Security Incident Response — Task Tracker

## 1. Overview

План действий при security incident.

## 2. Incident Types

| Severity | Examples |
|----------|----------|
| Critical | Data breach, RCE, mass token leak |
| High | Account takeover, privilege escalation |
| Medium | Suspicious login pattern, rate limit abuse |
| Low | Dependency vulnerability, scan finding |

## 3. Response Steps

### 3.1 Detection

- Automated: SIEM alerts, audit log anomalies, IDS.
- Manual: User reports, bug bounty.

### 3.2 Containment

- Отозвать скомпрометированные токены.
- Заблокировать пользователя/IP.
- Отключить уязвимый endpoint/feature flag.
- Изолировать инстанс (если RCE suspected).

### 3.3 Investigation

- Собрать логи: `audit_log`, nginx access, application logs.
- Определить scope: какие users/projects/data affected.
- Timeline восстановления.

### 3.4 Eradication

- Apply patch.
- Rotate secrets (JWT signing key, DB credentials, API tokens).
- Clean malicious data.

### 3.5 Recovery

- Verify patch in staging.
- Deploy to production.
- Monitor for recurrence.

### 3.6 Post-Incident

- Post-mortem within 48 hours.
- Update runbook.
- Notify affected users if required.

## 4. Specific Scenarios

### 4.1 JWT Secret Compromise

1. Rotate JWT signing key.
2. Invalidate all refresh tokens.
3. Force re-login all users.
4. Audit active sessions.

### 4.2 Database Dump Leak

1. Determine dump scope.
2. Rotate DB credentials.
3. Reset passwords affected users.
4. Review access logs.
5. Notify affected users.

### 4.3 Mass Token Reuse

1. Enable stricter rate limits.
2. Block source IPs.
3. Invalidate affected token families.
4. Review IP allowlists.

## 5. Communication

| Audience | Channel | Timing |
|----------|---------|--------|
| Internal team | Incident channel | Immediate |
| Users | Email / status page | After containment |
| Regulators | Per legal requirement | Per law |

## 6. Tools

- `audit_log` — primary source.
- Grafana dashboards — traffic anomalies.
- Loki — log search.
- `pgBadger` — DB access patterns.

## References

- `docs/SECURITY.md`
- `docs/AUTH_ADVANCED.md`
- `docs/MONITORING.md`
- `docs/OPS_RUNBOOK.md`
