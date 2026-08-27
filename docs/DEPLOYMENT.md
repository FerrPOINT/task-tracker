# Deployment — Task Tracker

## 1. Overview

Self-hosted таск-трекер. MVP поставляется как Docker Compose: backend (Rust), frontend (Vite static), PostgreSQL, Redis. Reverse proxy по желанию.

## 2. System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 4 GB | 8+ GB |
| Disk | 20 GB SSD | 100+ GB SSD |
| OS | Linux x86_64 | Ubuntu 22.04 LTS |
| Docker | 24.0+ | 27.0+ |
| Docker Compose | 2.20+ | 2.27+ |

## 3. Services

| Service | Image | Host port | Description |
|---------|-------|-----------|-------------|
| `frontend` | build from `frontend/Dockerfile` | `19877` | nginx статика |
| `backend` | build from `backend/Dockerfile` | `3456` | Axum API, non-root (uid 999) |
| `postgres` | `postgres:17.6-alpine` | внутренний | PostgreSQL, не публикуется |
| `redis` | `redis:8.0-alpine` | внутренний | Cache / event bus, не публикуется |
| `uploads-init` | `debian:bookworm-slim` | — | one-shot chown volume `uploads` |

## 4. Quick Start

```bash
cp .env.example .env
# ОБЯЗАТЕЛЬНО задайте POSTGRES_PASSWORD и TASKTRACKER_JWT_SECRET — compose
# откажется стартовать без них (рабочих дефолтов нет)
docker compose up -d --build
curl -sf http://localhost:3456/api/v1/health
```

## 5. Local Development

```bash
# Terminal 1
docker compose up -d postgres redis backend
cd backend && cargo run --bin server

# Terminal 2
cd frontend
pnpm install
pnpm generate:api
pnpm dev
```

Frontend dev-server проксирует `/api/v1` на backend `:3456` (см. `vite.config.ts`; переопределение — `VITE_API_BASE_URL`).

## 6. Production Build

```bash
cd frontend
pnpm install
pnpm generate:api
pnpm build
```

Результат — `frontend/dist`, который можно раздать nginx или встроить в контейнер.

## 7. Demo Credentials

- Email: `demo@example.com`
- Password: `demo`

Создаётся seed-миграцией при первом запуске backend.

## 8. Health Checks

| Endpoint | Service |
|----------|---------|
| `GET /api/v1/health` | api liveness |

## 9. Backup

```bash
./scripts/backup.sh backups/$(date +%F-%H%M)
```

См. [BACKUP_RESTORE](BACKUP_RESTORE.md).

## 10. Update

```bash
git pull origin main
docker compose build
docker compose up -d     # recreate подхватывает новый образ
```

Миграции применяются автоматически при старте backend. **Никогда не используйте `docker compose down -v`** для обновления — флаг `-v` удаляет volume с базой и attachments.

## 11. Reverse Proxy Example (nginx)

```nginx
server {
  listen 19877;

  location /api/ {
    proxy_pass http://127.0.0.1:3456;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
  }

  location / {
    root /var/www/task-tracker/frontend/dist;
    try_files $uri $uri/ /index.html;
  }
}
```

## References

- `docs/ARCHITECTURE.md`
- `docs/LOCAL_SETUP.md`
- `docs/OPS_RUNBOOK.md`
- `docs/SECURITY.md`
