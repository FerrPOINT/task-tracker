# Caching Strategy — Task Tracker

## 1. Overview

Кеширование используется для снижения нагрузки на PostgreSQL и ускорения повторяемых операций. Два уровня: in-memory (`moka`) и distributed (`redis`).

## 2. Cache Layers

| Layer | Library | Use Case | TTL |
|-------|---------|----------|-----|
| L1 in-memory | `moka` 0.12.15 | Частые локальные данные | 1-5 min |
| L2 distributed | `redis` 1.3.0 | Shared cache, multi-instance | 5-60 min |
| Query cache | TanStack Query | Frontend server state | по конфигурации |
| CDN / browser | Nginx/Vite | Static assets, attachments | long-term |

## 3. Backend Caching

### 3.1 Cache Key Convention

```
{namespace}:{entity}:{id}[:{version}]
```

Примеры:

- `tt:project:uuid`
- `tt:issue:uuid`
- `tt:board:uuid:config`
- `tt:jql:{hash}`
- `tt:reports:velocity:project_uuid:sprint_count`

### 3.2 What to Cache

| Data | Cache | TTL | Invalidation |
|------|-------|-----|--------------|
| Project by id/key | Redis | 10 min | on update/delete |
| Issue by id | Redis | 5 min | on update/delete |
| Board config | Redis | 10 min | on update |
| Workflow | Redis + moka | 15 min | on admin change |
| User profile | moka | 5 min | on update |
| Permissions matrix | moka | 5 min | on role change |
| JQL search results | Redis | 2 min | on any issue change |
| Reports | Redis | 1 hour | on data change |
| Issue type scheme | moka | 10 min | on admin change |

### 3.3 What NOT to Cache

- Пароли, токены, secrets.
- Данные с частыми writes и редкими reads.
- Большие бинарные файлы (их храним в S3/filesystem).

## 4. Cache Aside Pattern

```rust
async fn get_issue(&self, id: Uuid) -> Result<Issue, Error> {
    let key = format!("tt:issue:{id}")
    if let Some(cached) = self.cache.get(&key).await {
        return Ok(cached)
    }
    let issue = self.repo.find_by_id(id).await?
    self.cache.set(key, issue.clone(), TTL_5_MIN).await
    Ok(issue)
}
```

## 5. Write-Through / Invalidate

```rust
async fn update_issue(&self, id: Uuid, patch: PatchIssue) -> Result<Issue, Error> {
    let issue = self.repo.update(id, patch).await?
    self.cache.delete(format!("tt:issue:{id}")).await
    self.cache.delete_pattern("tt:jql:*").await
    self.event_bus.publish(IssueUpdated { id }).await
    Ok(issue)
}
```

## 6. Frontend Query Caching

### 6.1 Default Config

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

### 6.2 Per-Entity Stale Time

| Entity | Stale Time |
|--------|------------|
| User profile | 5 min |
| Project list | 2 min |
| Issue detail | 30 sec |
| Board | 10 sec |
| Reports | 1 hour |

## 7. WebSocket + Cache Invalidation

- При событии `issue_updated` frontend инвалидирует query key issue и связанные списки.
- Backend инвалидирует `tt:jql:*` и `tt:issue:{id}`.

## 8. Rate Limit Cache

- `tower_governor` использует in-memory rate limiter.
- В distributed режиме — Redis cell или shared state.

## 9. Cache Warming

- При старте приложения кешируются частые данные: workflows, issue type schemes, active projects.
- Background job обновляет reports cache ежечасно.

## 10. Monitoring

- Hit/miss ratio по namespace.
- Cache size (moka) / memory usage (redis).
- Alerts при cache eviction rate > 50%.

## 11. Eviction and Limits

| Cache | Max Size | Eviction |
|-------|----------|----------|
| moka | 10 000 entries | LRU |
| redis | по конфигурации | allkeys-lru |

## 12. Cache Stampede Protection

- Используем probabilistic early expiration.
- Для expensive queries — singleflight pattern (moka `async-cache` или custom).

## 13. Configuration

```toml
[cache]
backend = "redis"  # "memory" | "redis"
redis_url = "redis://redis:6379"
ttl_seconds = 300
max_capacity = 10000
```

## 14. Anti-Patterns

- Не кешировать результаты мутаций.
- Не кешировать без invalidation стратегии.
- Не кешировать персональные данные на shared уровне.
- Не делать cache TTL больше допустимого time-to-inconsistent.
## References

- `docs/ARCHITECTURE.md`
- `docs/PERFORMANCE.md`
- `docs/DEPLOYMENT.md`
