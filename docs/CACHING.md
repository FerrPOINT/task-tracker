# Caching Strategy — Task Tracker

## 1. Overview

Бэкенд не имеет серверного кеша на уровне приложения (ни moka, ни redis не подключены к runtime-коду). Кеширование работает только на уровне frontend (TanStack Query) и PostgreSQL (shared buffers + page cache). Раздел описывает **текущую** стратегию и планы.

## 2. Frontend Query Caching

TanStack Query — единственный активный слой кеширования.

### 2.1 Default Config

```ts
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30 * 1000,
      gcTime: 5 * 60 * 1000,
      refetchOnWindowFocus: true,
      refetchOnReconnect: true,
    },
  },
})
```

### 2.2 Per-Entity Stale Time

| Entity | Stale Time |
|--------|------------|
| User profile | 5 min |
| Project list | 2 min |
| Issue detail | 30 sec |
| Board | 10 sec |
| Reports | 1 hour |

### 2.3 SSE-Driven Invalidation

При событии `tracker` (SSE stream `issue_updated`, `issue_moved`, `sprint_changed` и т.д.) frontend инвалидирует соответствующие query keys.

## 3. Backend (No Application Cache)

- Все запросы идут напрямую в PostgreSQL через SeaORM.
- `moka` 0.12 декларирована в `backend/Cargo.toml`, но infra-cache модуль удалён — зависимость висит и может быть убрана.
- Redis присутствует в `docker-compose.yml` как сервис, но бэкенд к нему не подключён (нет `redis` crate в зависимостях).

## 4. Rate Limit Cache

- `tower_governor` использует in-memory rate limiter (per-instance, не distributed).

## 5. Planned (Not Implemented)

| Layer | Library | Use Case | TTL |
|-------|---------|----------|-----|
| L1 in-memory | `moka` | Частые локальные данные (workflow, user profile) | 1-5 min |
| L2 distributed | `redis` | Shared cache, multi-instance | 5-60 min |

### 5.1 Planned Cache Key Convention

```
{namespace}:{entity}:{id}[:{version}]
```

Примеры: `tt:project:uuid`, `tt:issue:uuid`, `tt:jql:{hash}`

### 5.2 Planned Cache Aside Pattern

```rust
async fn get_issue(&self, id: Uuid) -> Result<Issue, Error> {
    let key = format!("tt:issue:{id}");
    if let Some(cached) = self.cache.get(&key).await {
        return Ok(cached);
    }
    let issue = self.repo.find_by_id(id).await?;
    self.cache.set(key, issue.clone(), TTL_5_MIN).await;
    Ok(issue)
}
```

## 6. What NOT to Cache

- Пароли, токены, secrets.
- Данные с частыми writes и редкими reads.
- Большие бинарные файлы (их храним в filesystem / S3-compatible storage).

## References

- `docs/ARCHITECTURE.md`
- `docs/PERFORMANCE.md`