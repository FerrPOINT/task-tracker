# API Versioning — Task Tracker

## 1. Overview

API версионируется path-based: `/api/v1/...`. Версия меняется только при breaking changes. New features и additive changes добавляются в текущую версию.

## 2. Version Policy

| Change Type | Version Impact | Example |
|-------------|----------------|---------|
| Additive (new fields, endpoints) | Same version | New query param |
| Behavioral (different defaults) | Same or new version | Pagination size |
| Breaking (removed fields, renamed) | New version required | Remove endpoint |
| Security hardening | Same or new version | New required header |

## 3. Breaking Changes

Breaking change требует новой версии:

- Удаление endpoint.
- Удаление/переименование обязательного поля request/response.
- Изменение семантики HTTP status code.
- Изменение auth flow.

## 4. Deprecation

- Deprecated endpoint помечается в OpenAPI `deprecated: true`.
- `Sunset` header на deprecated endpoint.
- Минимум 6 месяцев поддержки после deprecation.
- Логи использования deprecated endpoints.

## 5. Client Headers

```
Accept: application/json
X-API-Version: 2026-07-13  # optional date-based feature flag
```

## 6. Backward Compatibility

- Response JSON: игнорировать неизвестные поля (never fail on extra fields).
- Query params: неизвестные параметры игнорируются с warning.
- Enum values: client должен обрабатывать fallback для новых значений.

## 7. Version Discovery

```
GET /api/v1/meta
```

Response:

```json
{
  "versions": ["v1"],
  "current": "v1",
  "deprecated": [],
  "sunset": null
}
```

## 8. Migration Guide

При выпуске `v2`:

- Создать `docs/API_V2_MIGRATION.md` с mapping endpoint/field.
- Breaking changes changelog.
- Script для проверки использования deprecated features.

## References

- `docs/API.md`
- `docs/ARCHITECTURE.md`
- `CHANGELOG.md` (корень репозитория)
