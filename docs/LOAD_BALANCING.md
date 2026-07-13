# Load Balancing & Scaling — Task Tracker

## 1. Overview

Как масштабировать приложение и распределять трафик между инстансами.

## 2. Deployment Topology

```
Users
  → Traefik / nginx (load balancer)
    → backend-instance-1 (HTTP + WS)
    → backend-instance-2 (HTTP + WS)
    → backend-instance-n
  → PostgreSQL (primary + replicas)
  → Redis (cluster / sentinel)
```

## 3. HTTP Load Balancing

- Round-robin между backend instances.
- Health check: `/health/ready`.
- Sticky sessions **не требуются** — stateless JWT.

## 4. WebSocket Scaling

### 4.1 Problem

- WebSocket connection привязан к одному инстансу.
- Если инстанс падает — соединение обрывается.
- Если событие происходит на инстансе A, а клиент на инстансе B — клиент не получит push.

### 4.2 Solution: Redis Pub/Sub

- Каждый backend instance подписан на Redis channel `events`.
- При возникновении события публикуем в Redis.
- Все instances получают событие и рассылают своим WS клиентам.

### 4.3 Reconnection

- Frontend reconnect с exponential backoff.
- После reconnect: `subscribe` на нужные channels + `sync` для missed events.

## 5. Background Jobs

- `apalis` workers распределяются по instances.
- Jobs должны быть idempotent.
- Redis-backed storage.
- Для избежания duplicate processing используем job idempotency key.

## 6. Database Scaling

| Pattern | When |
|---------|------|
| Read replicas | JQL/search load |
| Connection pooler (PgBouncer) | >100 connections |
| Vertical scaling | First growth phase |
| Sharding | Enterprise/multi-tenant |

## 7. Caching Layers

| Layer | Scope | Invalidation |
|-------|-------|--------------|
| L1 moka | Per instance | TTL / event |
| L2 Redis | Shared | tag-based |
| Browser | Static assets | hash in filename |
| CDN | Public assets | manual purge |

## 8. Rate Limiting at Edge

- Traefik plugin или nginx limit_req.
- Fallback на backend Redis token bucket.

## 9. Scaling Signals

| Metric | Threshold | Action |
|--------|-----------|--------|
| CPU > 70% | 5 min | Scale up backend |
| P95 latency > 500ms | 3 min | Add instance |
| WS connections / instance > 10k | - | Add instance |
| Queue depth > 1000 | - | Add workers |
| DB connections > 80% | - | Add pooler/replica |

## 10. Zero-Downtime Deploy

1. Новая версия поднимается рядом.
2. Startup probe green.
3. LB включает новые instances.
4. Старым instances посылается `SIGTERM`.
5. Graceful shutdown (30s drain).
6. Старые containers удаляются.

## References

- `docs/DEPLOYMENT.md`
- `docs/RUNTIME.md`
- `docs/CACHING.md`
- `docs/MONITORING.md`
- `docs/WEBSOCKET_EVENTS.md`
