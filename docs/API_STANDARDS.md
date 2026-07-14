# API Standards

## 1. Общие принципы

- API — REST поверх HTTP/1.1 и HTTP/2.
- Формат обмена данными — JSON.
- Кодировка — UTF-8.
- Версионирование — URL path: `/api/v1/...`. Подробнее в `docs/API_VERSIONING.md`.
- Язык по умолчанию — русский в UI, английский в технических полях и кодах ошибок.

## 2. OpenAPI

- Спецификация генерируется из кода через `utoipa-axum`.
- Источник правды — Rust handlers и dto-структуры.
- Swagger UI доступен по `/api/v1/docs` в dev-режиме.
- На каждый endpoint требуется:
  - summary
  - description (если бизнес-логика нетривиальна)
  - request body schema
  - response schemas + статус-коды
  - примеры ошибок `422`, `409`, `403`, `404`

## 3. URL и ресурсы

- Имена ресурсов — множественное число существительных: `/projects`, `/issues`.
- Иерархия через вложенность:
  - `/projects/{id}/members`
  - `/projects/{id}/issues`
  - `/issues/{id}/comments`
- UUID ресурсов — UUIDv7, строка в формате `xxxxxxxx-xxxx-7xxx-yxxx-xxxxxxxxxxxx`.

## 4. HTTP методы

| Метод | Операция | Семантика |
|-------|----------|-----------|
| GET | Чтение | идемпотентный, без side effects |
| POST | Создание | возвращает `201 Created` + `Location` |
| PUT | Полная замена | идемпотентный |
| PATCH | Частичное обновление | JSON Merge Patch по умолчанию |
| DELETE | Удаление | возвращает `204 No Content` или `202 Accepted` для soft delete |

## 5. Пагинация

- Для списков — cursor-based pagination по умолчанию.
- Параметры: `cursor`, `limit` (max 100, default 20).
- Альтернатива — offset-based для небольших справочников (`page`, `per_page`).
- Подробнее в `docs/PAGINATION.md`.

## 6. Сортировка и фильтрация

- Сортировка: `?sort=-created_at,title`.
- Фильтры: `?status=open&assignee_id=eq:uuid`.
- Поиск по тексту: `?q=keyword`.

## 7. Формат ошибок

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Request validation failed",
    "details": [
      { "field": "title", "message": "required" }
    ],
    "request_id": "req_01J..."
  }
}
```

Подробнее в `docs/ERROR_HANDLING.md`.

## 8. WebSocket

- Real-time события — отдельный WebSocket endpoint `/ws/v1`.
- Структура сообщений зафиксирована в `docs/WEBSOCKET_EVENTS.md`.

## 9. Rate limiting

- Заголовки ответа:
  - `X-RateLimit-Limit`
  - `X-RateLimit-Remaining`
  - `X-RateLimit-Reset`
- HTTP 429 при превышении лимита.

## 10. Идемпотентность

- Операции, чувствительные к дублированию (`POST /issues`, `POST /comments`), поддерживают заголовок `Idempotency-Key`.
- Ключ — UUIDv4, хранится в Redis 24 часа.

## 11. References

- `docs/API.md` — полный endpoint catalog.
- `docs/API_VERSIONING.md` — политика версионирования.
- `docs/ERROR_HANDLING.md` — формат ошибок.
- `docs/PAGINATION.md` — пагинация и bulk-операции.
- `docs/WEBSOCKET_EVENTS.md` — realtime events.
- `docs/SECURITY.md` — auth, CORS, CSRF.
