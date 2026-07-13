# Data Retention & Archiving — Task Tracker

## 1. Overview

Политики хранения, удаления и архивации данных.

## 2. Soft Delete

Таблицы с `deleted_at`:

- `projects`
- `issues`
- `comments`
- `attachments`
- `filters`
- `boards`

### 2.1 Behavior

- DELETE endpoint устанавливает `deleted_at`.
- Записи исключаются из обычных SELECT (where deleted_at is null).
- Trash UI показывает удалённые записи владельцу/admin.
- Восстановление возможно в течение retention period.

## 3. Retention Periods

| Data Type | Soft Delete Period | Hard Delete |
|-----------|-------------------|-------------|
| Issues | 30 days | After retention |
| Projects | 30 days | After retention |
| Comments | 30 days | After retention |
| Attachments | 30 days | After retention + S3 lifecycle |
| Audit log | 1 year | Archive to cold storage |
| Changelog | 2 years | Archive to cold storage |
| Notification deliveries | 90 days | Hard delete |
| Dead-letter jobs | 30 days | Hard delete |
| Password reset tokens | 1 hour | Hard delete |
| Email verification tokens | 24 hours | Hard delete |
| Refresh tokens | On logout / rotation | Hard delete |
| API tokens | On expiration / revoke | Hard delete |

## 4. Archiving

- Audit log и changelog partitionированы по месяцам.
- partitions старше retention period:
  - сжимаются в Parquet/ORC.
  - переносятся в object storage.
  - удаляются из primary DB.
- Архив доступен через read-only API admin UI.

## 5. GDPR / Data Deletion

- Endpoint `DELETE /api/v1/users/me` — полное удаление пользователя.
- Каскадная анонимизация:
  - issues: assignee/reporter set to null, comments anonymized.
  - audit log entries сохраняются с `user_id = null`.
- Export user data: `GET /api/v1/users/me/export`.

## 6. Backup Strategy

| Backup Type | Frequency | Retention |
|-------------|-----------|-----------|
| Full DB dump | Daily | 30 days |
| WAL archive | Continuous | 7 days |
| File storage | Cross-region replication | 30 days |
| Redis | Daily RDB | 7 days |

## 7. Purge Job

- Запускается раз в сутки через `apalis`.
- Удаляет истёкшие токены.
- Архивирует старые audit/changelog partitions.
- Очищает trash после retention period.
- Логирует количество удалённых записей.

## References

- `docs/DATA_MODEL.md`
- `docs/SECURITY.md`
- `docs/MONITORING.md`
- `docs/OPS_RUNBOOK.md`
