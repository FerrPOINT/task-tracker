# Operations Runbook — Task Tracker

## 1. Overview

Пошаговые инструкции для типовых операций production-инстанса: deploy, rollback, backup, restore, incident response.

## 2. Daily Checks

```bash
docker compose ps
docker compose logs --tail 100 api
curl -f https://tasktracker.example.com:19876/health/ready
curl -f https://tasktracker.example.com:19876/metrics | grep up
```

## 3. Deploy New Version

```bash
cd /opt/task-tracker
git fetch origin
git checkout main
git pull origin main
docker compose build
docker compose up -d
docker compose run --rm migrator
docker compose ps
```

## 4. Rollback

```bash
# Revert code
git log --oneline -20
git revert <bad-commit>
docker compose build
docker compose up -d

# DB rollback: apply down-migration (если есть)
docker compose run --rm migrator down
```

## 5. Backup

```bash
./scripts/backup.sh
# Verify archive in /backups
ls -lh /backups
```

## 6. Restore

```bash
# Stop app
docker compose stop api

# Restore DB
./scripts/restore.sh /backups/task-tracker-YYYY-MM-DD.tar.gz

# Restart
docker compose up -d
```

## 7. Scaling API

```bash
docker compose up -d --scale api=3
```

## 8. High CPU / Memory

1. Check `top` / `docker stats`.
2. Review slow query log.
3. Restart affected container.
4. Enable rate limit if DDoS suspected.

## 9. DB Connection Pool Exhaustion

```sql
SELECT count(*), state FROM pg_stat_activity GROUP BY state;
```

Mitigation:

- Restart API pods.
- Increase pool size temporarily.
- Kill long-running queries.

## 10. Redis Failure

- Switch to single-instance mode temporarily (`TASKTRACKER_REDIS_URL` → localhost fallback).
- Rebuild Redis slave.
- WebSocket real-time будет задерживаться.

## 11. Disk Full

```bash
df -h
docker system prune -a --volumes  # careful
./scripts/cleanup_old_backups.sh
```

## 12. Incident Contacts

- On-call: ...
- Slack channel: #alerts
- PagerDuty: ...

## 13. Post-Mortem

After every SEV-1/SEV-2 incident:

1. Timeline.
2. Root cause.
3. Impact.
4. Remediation.
5. Preventive actions.

## References

- `docs/DEPLOYMENT.md`
- `docs/MONITORING.md`
- `docs/SECURITY.md`
- `docs/MIGRATIONS.md`
