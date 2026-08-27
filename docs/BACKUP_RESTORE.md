# Backup & Restore — Task Tracker

## 1. Что входит в бэкап

- PostgreSQL: `pg_dump -Fc` (custom format, восстановление через `pg_restore`).
- Attachments: содержимое Docker volume `uploads` (tar.gz, с сохранением прав).

Скрипты: `scripts/backup.sh` (создание), `scripts/restore.sh` (восстановление), `scripts/cleanup_old_backups.sh` (ротация).

## 2. Автоматизация

 cron-пример (ежедневно в 03:15, хранить 14 копий):

```cron
15 3 * * * cd /opt/dev/task-tracker && ./scripts/backup.sh >> backups/backup.log 2>&1 && ./scripts/cleanup_old_backups.sh 14
```

## 3. Ручной бэкап

```bash
./scripts/backup.sh backups/manual-$(date +%F)
# Контроль: в архиве два файла — <имя>.dump и <имя>-attachments.tar.gz
tar -tzf backups/manual-*.tar.gz
```

## 4. Восстановление

```bash
docker compose stop backend frontend
./scripts/restore.sh backups/task-tracker-YYYY-MM-DD-HHMMSS.tar.gz
docker compose up -d
curl -f http://localhost:3456/api/v1/health
```

`restore.sh`:

1. распаковывает архив;
2. `pg_restore --clean --if-exists` в базу из `TASKTRACKER_DATABASE__URL` / переменных `.env`;
3. восстанавливает attachments в volume `uploads` и делает `chown 999:999` (backend работает non-root).

## 5. Point-in-time recovery

WAL-архивирование не настроено по умолчанию. Для PITR подключите внешний инструмент (pgBackRest, WAL-G) к volume `postgres_data`.

## References

- [OPS_RUNBOOK](OPS_RUNBOOK.md)
- [STORAGE](STORAGE.md)
- [DEPLOYMENT](DEPLOYMENT.md)
