# Operations Runbook — Task Tracker

Пошаговые инструкции для типовых операций production-инстанса: deploy, rollback, backup, restore, инциденты.

Все команды выполняются из корня репозитория (`/opt/dev/task-tracker` или ваш путь деплоя).

## 2. Ежедневные проверки

```bash
docker compose ps          # все сервисы должны быть Up (healthy)
docker compose logs --tail 100 backend
curl -f http://localhost:3456/api/v1/health     # без rate-limit
curl -f http://localhost:3456/metrics | head    # Prometheus-метрики, если TASKTRACKER_METRICS__PUBLIC=true
```

Для production-инстансов с публичным backend endpoint закройте `/metrics` на proxy/edge или задайте `TASKTRACKER_METRICS__PUBLIC=false`.

Frontend: `curl -f http://localhost:19877/` → 200.

## 3. Деплой новой версии

```bash
git fetch origin && git checkout main && git pull origin main
docker compose build
docker compose up -d        # recreate: подхватывает новый образ
docker compose ps
```

Миграции применяются автоматически при старте backend-контейнера. Отдельного сервиса `migrator` в compose нет.

## 4. Rollback

```bash
git log --oneline -20
git revert <bad-commit>     # или checkout предыдущего тега
docker compose build
docker compose up -d
```

Down-миграции не поставляются; откат схемы БД — восстановлением из бэкапа (см. §6).

## 5. Бэкап

```bash
./scripts/backup.sh [путь-без-расширения]
# создает <имя>.tar.gz: pg_dump (-Fc) + attachments из Docker volume `uploads`
ls -lh backups/
```

## 6. Восстановление

```bash
docker compose stop backend frontend
./scripts/restore.sh backups/task-tracker-<дата>.tar.gz
docker compose up -d
curl -f http://localhost:3456/api/v1/health
```

`restore.sh` восстанавливает и БД (pg_restore `--clean --if-exists`), и attachments (в volume `uploads`, с chown под non-root backend uid 999).

## 7. Масштабирование

```bash
docker compose up -d --scale backend=3
```

Frontend/Postgres не масштабируются (статика и single-writer БД). Перед масштабированием backend вынесите `uploads` в shared storage.

## 8. Высокий CPU / память

```bash
docker stats
docker compose logs backend | grep -E 'ERROR|panic' | tail -20
```

## 9. Инциденты

| Симптом | Диагностика | Действие |
|---|---|---|
| backend unhealthy | `docker compose logs backend` | проверить БД/секреты; `docker compose up -d backend` |
| 429 на API | общие лимиты 60/60с на IP | поднять `TASKTRACKER_SERVER__GENERAL_RATE_*` в `.env` |
| Логин не работает | `docker compose logs backend \| grep -i auth` | проверить JWT-секрет не изменился |
| Потеря attachments | `docker volume inspect task-tracker_uploads` | восстановить из бэкапа (§6) |

## References

- [LOCAL_SETUP](LOCAL_SETUP.md)
- [BACKUP_RESTORE](BACKUP_RESTORE.md)
- [MONITORING](MONITORING.md)
- [TROUBLESHOOTING](TROUBLESHOOTING.md)
