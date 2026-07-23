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

| Service | Image | Port | Description |
|---------|-------|------|-------------|
| `backend` | build from `backend/Dockerfile` | `3456` | Axum API |
| `postgres` | `postgres:17.6-alpine` | `5432` | PostgreSQL |
| `redis` | `redis:8.0-alpine` | `6379` | Cache / event bus |

## 4. Quick Start

```bash
cp backend/.env.example backend/.env
# отредактируйте секреты
docker compose up -d postgres redis backend
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

Frontend dev-server ожидает backend по `http://127.0.0.1:3456/api/v1` (env `VITE_API_BASE_URL`).

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
docker compose exec -T postgres pg_dump -U tasktracker tasktracker > tasktracker-$(date +%Y%m%d).sql
docker compose cp task-tracker-backend-1:/data/attachments ./attachments-backup
```

## 10. Update

```bash
git pull origin main
docker compose down -v   # при изменениях миграций
docker compose up -d postgres redis backend
```

## 11. Reverse Proxy Example (nginx)

```nginx
server {
  listen 19876;

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
