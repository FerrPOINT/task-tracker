# Database Standards

> Стартовый документ. До конца разработки могут измениться конкретные индексы, именование и стратегия миграций — актуализировать при стабилизации схемы.

## 1. СУБД

- PostgreSQL 16+.
- Тип UUID — `UUID` (pgcrypto / uuid-ossp). PK по умолчанию `gen_random_uuid()`.
- Для новых PK рекомендуется UUIDv7, но пока допустим UUIDv4.

## 2. Миграции

- Фреймворк — `sqlx migrate` / `refinery` (решение зафиксировано в ADR).
- Имя файла: `YYYYMMDDHHMMSS_description.sql`.
- Каждая миграция:
  - оборачивается в `BEGIN; ... COMMIT;`
  - имеет `ROLLBACK` в down-файле
  - не удаляет данные без `WHERE` и бэкапа
- Запрещено:
  - изменять уже применённую миграцию
  - удалять столбцы с данными без explicit migration step
  - использовать `SELECT *` в миграциях

## 3. Именование

| Объект | Конвенция | Пример |
|---|---|---|
| Таблица | snake_case, множественное число | `issue_status_history` |
| Столбец | snake_case | `created_at` |
| PK | `id` | `id UUID PRIMARY KEY` |
| FK | `<table>_id` | `project_id` |
| Индекс | `<table>_<columns>_idx` | `issues_project_status_idx` |
| Constraint | `<table>_<columns>_<type>` | `issues_project_key_unique` |
| Enum | `enum_<name>` | `enum_issue_priority` |

## 4. Типы данных

| Назначение | Тип | Примечание |
|---|---|---|
| ID | `UUID` | PK default `gen_random_uuid()` |
| Timestamp | `TIMESTAMPTZ` | всегда UTC |
| JSON | `JSONB` | для неструктурированных/расширяемых данных |
| Перечисления | `TEXT` + check / native `enum` | для маленьких стабильных списков — native enum; для часто меняющихся — lookup table |
| Деньги | `NUMERIC(19,4)` | если потребуется |
| Длительность | `INTERVAL` | worklogs, SLA |
| IP | `INET` | audit log |

## 5. Индексы

- Каждый FK — индекс.
- Частые фильтры и сортировки — покрывающие индексы.
- GIN для `JSONB` полей, по которым идёт поиск.
- Уникальные индексы для бизнес-ключей (`project_key_unique`).

## 6. Soft delete

- По умолчанию — hard delete.
- Для критичных сущностей (`issues`, `projects`) — `deleted_at TIMESTAMPTZ` + partial unique index.
- Корзина реализована через `issue_trash`.

## 7. Constraints

- `NOT NULL` по умолчанию для обязательных полей.
- `DEFAULT` только для технических полей (`created_at`, `id`).
- `ON DELETE`:
  - `CASCADE` — для явно дочерних сущностей (comments к issue)
  - `RESTRICT` — для ссылок на справочники, если удаление нарушает целостность
  - `SET NULL` — для опциональных FK (`assignee_id`)

## 8. Partitioning

- Кандидаты на партиционирование:
  - `audit_log` — по `created_at` (range)
  - `activity_log` — по `created_at` (range)
  - `worklogs` — по `created_at` (range)
- До 10M+ строк не партиционируем.

## 9. SQL style

- Ключевые слова — uppercase.
- Идентификаторы — lowercase.
- Запросы форматировать с переносами:

```sql
SELECT i.id, i.summary, s.name AS status
FROM issues i
JOIN statuses s ON s.id = i.status_id
WHERE i.project_id = $1
  AND i.deleted_at IS NULL
ORDER BY i.created_at DESC
LIMIT 50;
```

## 10. Seeds и fixtures

- Seed-данные для dev — `backend/migrations/seeds/`.
- Fixtures для тестов — `backend/tests/fixtures/`.
- Продакшен defaults (admin user, base issue types/statuses) — через миграцию `00000000000000_baseline.sql`.

## 11. References

- `docs/DATA_MODEL.md` — полная физическая модель.
- `docs/MIGRATIONS.md` — процесс миграций.
- `docs/DATABASE_INDEXES.md` — перечень индексов.
- `docs/ARCHITECTURE.md` — persistence layer.
