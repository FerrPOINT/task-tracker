# Backup & Restore

> Стартовый документ. До конца разработки скрипты и процедуры могут измениться — актуализировать при стабилизации инфраструктуры.

## 1. Что бэкапим

| Компонент | Способ | Частота |
|---|---|---|
| PostgreSQL | `pg_dump` | ежедневно |
| Attachments | `rsync` / object storage replication | ежедневно |
| Redis | необязательно (cache + pub/sub) | — |
| `.env` | внешний secret manager / encrypted store | при изменении |

## 2. Автоматический бэкап

```bash
./scripts/backup.sh
```

Скрипт делает:

1. `pg_dump` в `/backups/postgres-YYYY-MM-DD.sql.gz`.
2. `rsync` attachments в `/backups/attachments/`.
3. Архив `/backups/task-tracker-YYYY-MM-DD.tar.gz`.
4. Ротация: хранить последние 7 дневных и 4 недельных снапшота.

### Cron

```cron
0 2 * * * cd /opt/dev/task-tracker && ./scripts/backup.sh >> /var/log/tasktracker-backup.log 2>&1
```

## 3. Ручной бэкап

```bash
# PostgreSQL
docker compose exec postgres pg_dump -U tasktracker tasktracker | gzip > tasktracker-$(date +%F).sql.gz

# Attachments
docker compose cp api:/data/attachments ./attachments-backup
```

## 4. Восстановление

```bash
./scripts/restore.sh /backups/task-tracker-2026-07-13.tar.gz
```

Порядок:

1. Остановить `api` и `frontend`.
2. Восстановить Postgres:
   ```bash
   gunzip -c postgres-2026-07-13.sql.gz | docker compose exec -T postgres psql -U tasktracker -d tasktracker
   ```
3. Восстановить attachments.
4. Запустить `api` и проверить `/health/ready`.

## 5. Point-in-time recovery

- Если включён WAL archiving — восстановление до момента времени.
- Нужен отдельный backup solution (Barman, pgBackRest, WAL-G).

## 6. Object storage backup

Если attachments в S3/MinIO:

- Включить bucket versioning.
- Настроить cross-region replication.

## 7. Проверка бэкапов

- Раз в месяц делать test restore на staging.
- Метрика: `backup_last_success_timestamp`.

## 8. Disaster recovery

| Сценарий | RTO | RPO | Действия |
|---|---|---|---|
| Потеря данных PG | 1 час | 24 часа | restore из последнего pg_dump |
| Потеря attachments | 30 мин | 24 часа | rsync из бэкапа или S3 |
| Потеря entire host | 4 часа | 24 часа | развернуть на новом хосте из бэкапа |

## 9. References

- `docs/DEPLOYMENT.md`
- `docs/OPS_RUNBOOK.md`
- `docs/MONITORING.md`
- `docs/DATA_RETENTION.md`
