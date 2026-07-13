# Deployment — Task Tracker

## 1. Overview

Task Tracker — self-hosted приложение. Поставляется как Docker Compose стек: backend (Rust), frontend (Vite static + Nginx), PostgreSQL, Redis, Traefik.

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
| `traefik` | `traefik:v3.4` | `80`, `443`, `19876` | Reverse proxy, TLS |
| `frontend` | build from `frontend/Dockerfile` | `3000` (internal) | Nginx serving static |
| `api` | build from `backend/Dockerfile` | `8080` (internal) | Axum backend |
| `postgres` | `postgres:17.6-alpine` | `5432` (internal) | PostgreSQL |
| `redis` | `redis:8.0-alpine` | `6379` (internal) | Cache + WS pub/sub |
| `migrator` | build from `backend/Dockerfile` | one-shot | DB migrations |

## 4. Files

```
├── docker-compose.yml
├── docker-compose.override.yml          # local overrides
├── docker-compose.prod.yml              # production overrides
├── .env.example
├── backend/Dockerfile
├── frontend/Dockerfile
├── frontend/nginx.conf
├── traefik/
│   ├── traefik.yml
│   └── dynamic.yml
└── scripts/
    ├── init.sh
    └── backup.sh
```

## 5. docker-compose.yml

```yaml
services:
  traefik:
    image: traefik:v3.4
    command:
      - "--api.insecure=false"
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.web.address=:80"
      - "--entrypoints.websecure.address=:443"
      - "--entrypoints.tasktracker.address=:19876"
      - "--certificatesresolvers.letsencrypt.acme.tlschallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.email=admin@example.com"
      - "--certificatesresolvers.letsencrypt.acme.storage=/letsencrypt/acme.json"
    ports:
      - "19876:19876"
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./traefik/letsencrypt:/letsencrypt
    networks:
      - tasktracker

  postgres:
    image: postgres:17.6-alpine
    environment:
      POSTGRES_USER: ${TASKTRACKER_DB_USER:-tasktracker}
      POSTGRES_PASSWORD: ${TASKTRACKER_DB_PASSWORD}
      POSTGRES_DB: ${TASKTRACKER_DB_NAME:-tasktracker}
    volumes:
      - postgres_data:/var/lib/postgresql/data
    networks:
      - tasktracker
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $${POSTGRES_USER} -d $${POSTGRES_DB}"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:8.0-alpine
    networks:
      - tasktracker
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5

  migrator:
    build:
      context: ./backend
      target: migrator
    environment:
      TASKTRACKER_DATABASE_URL: ${TASKTRACKER_DATABASE_URL}
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - tasktracker
    profiles:
      - migrator

  api:
    build:
      context: ./backend
      target: runtime
    environment:
      TASKTRACKER_DATABASE_URL: ${TASKTRACKER_DATABASE_URL}
      TASKTRACKER_REDIS_URL: ${TASKTRACKER_REDIS_URL}
      TASKTRACKER_SERVER_PORT: 8080
      TASKTRACKER_SERVER_HOST: 0.0.0.0
      TASKTRACKER_JWT_SECRET: ${TASKTRACKER_JWT_SECRET}
      TASKTRACKER_REFRESH_SECRET: ${TASKTRACKER_REFRESH_SECRET}
      TASKTRACKER_SMTP_HOST: ${TASKTRACKER_SMTP_HOST}
      TASKTRACKER_SMTP_PORT: ${TASKTRACKER_SMTP_PORT}
      TASKTRACKER_SMTP_USERNAME: ${TASKTRACKER_SMTP_USERNAME}
      TASKTRACKER_SMTP_PASSWORD: ${TASKTRACKER_SMTP_PASSWORD}
      TASKTRACKER_FILE_STORAGE_BACKEND: ${TASKTRACKER_FILE_STORAGE_BACKEND:-filesystem}
      TASKTRACKER_FILE_STORAGE_PATH: ${TASKTRACKER_FILE_STORAGE_PATH:-/data/attachments}
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
      migrator:
        condition: service_completed_successfully
    volumes:
      - tasktracker_data:/data/attachments
    networks:
      - tasktracker
    labels:
      - "traefik.enable=true"
      - "traefik.entrypoints=tasktracker"
      - "traefik.http.routers.api.rule=PathPrefix(`/api`) || PathPrefix(`/ws`) || PathPrefix(`/health`) || PathPrefix(`/metrics`)"
      - "traefik.http.services.api.loadbalancer.server.port=8080"

  frontend:
    build:
      context: ./frontend
      args:
        VITE_API_URL: ${VITE_API_URL:-/api/v1}
    depends_on:
      - api
    networks:
      - tasktracker
    labels:
      - "traefik.enable=true"
      - "traefik.entrypoints=tasktracker"
      - "traefik.http.routers.frontend.rule=PathPrefix(`/`)"
      - "traefik.http.services.frontend.loadbalancer.server.port=80"

volumes:
  postgres_data:
  tasktracker_data:

networks:
  tasktracker:
    driver: bridge
```

## 6. .env.example

```env
# Server
TASKTRACKER_SERVER_HOST=0.0.0.0
TASKTRACKER_SERVER_PORT=8080
TASKTRACKER_LOG_LEVEL=info

# Database
TASKTRACKER_DB_USER=tasktracker
TASKTRACKER_DB_PASSWORD=change_me_in_production
TASKTRACKER_DB_NAME=tasktracker
TASKTRACKER_DATABASE_URL=postgres://tasktracker:${TASKTRACKER_DB_PASSWORD}@postgres:5432/tasktracker

# Redis
TASKTRACKER_REDIS_URL=redis://redis:redis_password@redis:6379

# Auth
TASKTRACKER_JWT_SECRET=change_me_in_production_32bytes_min
TASKTRACKER_REFRESH_SECRET=change_me_in_production_32bytes_min
TASKTRACKER_ADMIN_EMAIL=admin@example.com
TASKTRACKER_ADMIN_PASSWORD=change_me_in_production

# Argon2id
TASKTRACKER_ARGON2_MEMORY_COST=65536
TASKTRACKER_ARGON2_TIME_COST=3

# CORS
TASKTRACKER_CORS_ALLOWED_ORIGINS=http://localhost:19876,http://127.0.0.1:19876

# SMTP (optional)
TASKTRACKER_SMTP_HOST=
TASKTRACKER_SMTP_PORT=587
TASKTRACKER_SMTP_USERNAME=
TASKTRACKER_SMTP_PASSWORD=***
TASKTRACKER_SMTP_FROM_ADDRESS=noreply@example.com

# File storage
TASKTRACKER_FILE_STORAGE_BACKEND=filesystem
TASKTRACKER_FILE_STORAGE_PATH=/data/attachments
# TASKTRACKER_FILE_STORAGE_ENDPOINT=
# TASKTRACKER_FILE_STORAGE_BUCKET=
# TASKTRACKER_FILE_STORAGE_ACCESS_KEY=
# TASKTRACKER_FILE_STORAGE_SECRET_KEY=
# TASKTRACKER_FILE_STORAGE_REGION=

# Frontend
VITE_API_URL=/api/v1
VITE_WS_URL=/ws/v1

# Traefik / TLS
TRAEFIK_ACME_EMAIL=admin@example.com
```

## 7. Local Development

```bash
cp .env.example .env
# edit .env
docker compose up -d postgres redis
cd backend && cargo run --bin server
cd frontend && pnpm dev
```

## 8. Production Deployment

```bash
cp .env.example .env
# fill secrets
./scripts/init.sh
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d --build
docker compose run --rm migrator
```

## 9. Health Checks

| Endpoint | Service |
|----------|---------|
| `GET /health` | api liveness |
| `GET /health/ready` | api readiness (DB + Redis) |
| `GET /metrics` | Prometheus metrics |

## 10. Updates

```bash
# Pull latest code
git pull origin main

# Rebuild and recreate containers
docker compose build
docker compose up -d

# Run migrations
docker compose run --rm migrator
```

## 11. Backup

```bash
./scripts/backup.sh
```

Скрипт делает:

- `pg_dump` PostgreSQL.
- `rsync` attachments.
- архив с timestamp в `/backups`.

## 12. Restore

```bash
./scripts/restore.sh /backups/task-tracker-2026-07-13.tar.gz
```

## 13. Reverse Proxy without Docker

Если Traefik не используется:

```nginx
server {
  listen 19876;
  server_name tasktracker.example.com;

  location /api/ {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
  }

  location /ws/ {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
  }

  location / {
    root /var/www/task-tracker/frontend/dist;
    try_files $uri $uri/ /index.html;
  }
}
```

## 14. Ports and Firewall

| Port | Purpose |
|------|---------|
| 19876 | Main application HTTP/HTTPS |
| 80/443 | Traefik HTTP/HTTPS (optional) |
| 5432 | PostgreSQL (internal only) |
| 6379 | Redis (internal only) |

## 15. TLS

- Local: self-signed cert или HTTP.
- Production: Let's Encrypt через Traefik.
- Internal: corporate CA cert.

## 16. Scaling

- `api` — horizontal scaling, stateless.
- `postgres` — primary + replica (pg_basebackup).
- `redis` — Redis Cluster или Sentinel.
- WebSocket — Redis pub/sub между instances.

## 17. Monitoring Stack (optional)

Дополнительные сервисы:

- Prometheus + Grafana (`docker-compose.monitoring.yml`).
- Loki для логов.
- Alertmanager.

## 18. Graceful Shutdown

Backend завершает работу корректно:

1. Получает `SIGTERM` / `SIGINT`.
2. Перестаёт принимать новые HTTP/WebSocket соединения.
3. Ждёт завершения активных запросов (` graceful shutdown timeout`, по умолчанию 30s).
4. Закрывает пул PostgreSQL и Redis.
5. Завершает фоновые задачи `apalis` (in-flight jobs завершаются, новые не берутся).
6. Закрывает соединения WebSocket с broadcast сообщением `server:shutdown`.

```yaml
# docker-compose.yml (api service)
stop_grace_period: 35s
```

```rust
// backend/src/main.rs (pseudo)
let server = axum::serve(listener, app);
tokio::select! {
    _ = server => {},
    _ = shutdown_signal => {},
}
// cleanup: close pools, stop workers
```

## 19. Multi-Environment

| Environment | Compose files |
|-------------|---------------|
| local | `docker-compose.yml` + `docker-compose.override.yml` |
| staging | `docker-compose.yml` + `docker-compose.staging.yml` |
| production | `docker-compose.yml` + `docker-compose.prod.yml` |
## References

- `docs/ARCHITECTURE.md`
- `docs/SECURITY.md`
- `docs/MONITORING.md`
- `docs/MIGRATIONS.md`
