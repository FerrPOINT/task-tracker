# Pagination & Bulk Operations — Task Tracker

## 1. Overview

Соглашения о пагинации и массовых операциях API.

## 2. Pagination Modes

### 2.1 Offset Pagination

Для маленьких/упорядоченных списков:

```json
GET /api/v1/projects?limit=20&offset=0

{
  "data": [...],
  "total": 145,
  "limit": 20,
  "offset": 0
}
```

### 2.2 Cursor Pagination

Для больших/динамических списков (JQL, activity, audit log):

```json
GET /api/v1/issues/search?cursor=eyJpZCI6InV1aWQ..."

{
  "data": [...],
  "nextCursor": "eyJpZCI6...",
  "hasMore": true
}
```

- Cursor кодируется base64(JSON).
- Содержит последнее значение sort key + id.
- Не поддерживает произвольный page jump.

### 2.3 Keyset Pagination

Для kanban columns / ordered lists:

- `?afterId=uuid&limit=20`
- Сортировка по `position` + `id`.

## 3. Limits

| Resource | Max Limit | Default |
|----------|-----------|---------|
| projects | 100 | 20 |
| issues | 100 | 20 |
| comments | 50 | 20 |
| audit log | 200 | 50 |
| JQL results | 1000 | 50 |
| users | 100 | 20 |
| attachments | 50 | 20 |

## 4. Bulk Operations

### 4.1 Bulk Create Issues

```json
POST /api/v1/issues/bulk
Idempotency-Key: uuid

{
  "issues": [
    { "projectKey": "PROJ", "summary": "...", "issueType": "task" },
    ...
  ]
}
```

### 4.2 Bulk Update

```json
PATCH /api/v1/issues/bulk
{
  "ids": ["uuid1", "uuid2"],
  "set": {
    "statusId": "uuid-done",
    "assigneeId": "uuid-user"
  }
}
```

### 4.3 Bulk Delete / Move to Trash

```json
POST /api/v1/issues/bulk/delete
{
  "ids": ["uuid1", "uuid2"]
}
```

### 4.4 Response Format

```json
{
  "processed": 100,
  "succeeded": 98,
  "failed": 2,
  "errors": [
    { "id": "uuid1", "error": "permission_denied" }
  ]
}
```

## 5. Request Size Limits

| Type | Limit |
|------|-------|
| JSON body | 10 MB |
| Bulk items | 100 |
| Attachments total | 50 MB |
| Query params length | 4096 chars |

## 6. Deep Linking

- Kanban filters, JQL, column widths кодируются в URL query.
- При refresh страницы состояние восстанавливается.

## 7. Performance

- Offset pagination ограничен `limit ≤ 100`.
- Для больших выборок — cursor или keyset.
- `COUNT(*)` не выполняется для cursor responses.
- `total` доступен только для offset mode.

## References

- `docs/API.md`
- `docs/PERFORMANCE.md`
- `docs/TESTING.md`
