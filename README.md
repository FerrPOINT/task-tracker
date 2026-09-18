<p align="center">
  <img src="docs/assets/task-tracker-readme-banner.svg" alt="Base Task Tracker - planning, kanban, issues and reporting" />
</p>

<p align="center">
  <a href="#capabilities"><img src="https://img.shields.io/badge/Capabilities-1d4ed8?style=for-the-badge" alt="Capabilities" /></a>
  <a href="#quick-start"><img src="https://img.shields.io/badge/Quick_Start-1e40af?style=for-the-badge" alt="Quick start" /></a>
  <a href="#visual-proof"><img src="https://img.shields.io/badge/Visual_Proof-0f766e?style=for-the-badge" alt="Visual proof" /></a>
  <a href="#safety"><img src="https://img.shields.io/badge/Safety-155e75?style=for-the-badge" alt="Safety" /></a>
  <a href="#quality"><img src="https://img.shields.io/badge/Quality-334155?style=for-the-badge" alt="Quality" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/Axum-Rest_API-1d4ed8?style=flat-square" alt="Axum REST API" />
  <img src="https://img.shields.io/badge/PostgreSQL-17-4169e1?style=flat-square&logo=postgresql&logoColor=white" alt="PostgreSQL 17" />
  <img src="https://img.shields.io/badge/Redis-8-dc2626?style=flat-square&logo=redis&logoColor=white" alt="Redis 8" />
  <img src="https://img.shields.io/badge/React-19-38bdf8?style=flat-square&logo=react&logoColor=0f172a" alt="React 19" />
  <img src="https://img.shields.io/badge/CI-.github%2Fworkflows%2Fci.yml-15803d?style=flat-square" alt="Repository CI" />
</p>

> **Base Task Tracker** is a self-hosted project-operations application for projects, issues, kanban, planning, work records and reports. It is a product MVP/hardening branch, not a hosted multi-tenant service.

<a name="overview"></a>
## Overview

Task Tracker combines project and issue workflows with an API-backed React UI and an HTTP-only `task-tracker` CLI. Its public contract is [openapi/openapi.json](openapi/openapi.json); the backend is a Rust workspace with PostgreSQL persistence, Redis-backed runtime services and a non-root attachment volume.

| Surface | Current behavior | Boundary |
|---|---|---|
| Project work | Projects, members, issues, comments, attachments, issue links, labels and search. | PostgreSQL and Redis are Compose-internal by default. |
| Planning | Boards, backlog, sprints, worklogs and velocity/burndown/cumulative-flow/control-chart reports. | Reports describe tracker data; they do not run pipeline or deployment jobs. |
| Delivery signals | In-app notifications, SSE, email-digest settings and audit records. | Email/ingress policy remains deployment-owned. |
| Administration | Users, instance settings, security headers, rate limits and optional public Prometheus metrics. | Auth, CORS, cookies, TLS and reverse proxy require an operator review for shared deployments. |
| Interfaces | React SPA, CLI and generated OpenAPI client. | All clients use the same `/api/v1` API boundary. |

The implementation and target architecture are indexed in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/TZ.md](docs/TZ.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

<a name="capabilities"></a>
## Capabilities

- **Issues in context.** Create and move issues, add comments and attachments, track watchers/votes, relate issues and capture worklogs.
- **Planning surfaces.** Work with boards, backlog and sprints; inspect velocity, burndown, cumulative flow and control charts.
- **Project metadata.** Maintain priorities, labels, issue types, custom fields, components, versions and release metadata.
- **Safe operations.** Use role-aware API/UI boundaries, request-rate controls, security headers, audit records and health/metrics endpoints.
- **Identity bridge.** In the Base umbrella runtime, configured central-auth tokens can be validated and central login can be proxied; local authentication remains the fallback where that bridge is absent.

<a name="quick-start"></a>
## Quick Start

The repository Compose file intentionally has no usable database-password or JWT defaults. Copy the template, supply operator-owned secrets, and keep `.env` ignored.

```bash
cp .env.example .env
# Edit .env: set POSTGRES_PASSWORD and TASKTRACKER_JWT_SECRET.
docker compose up --build -d
curl -fsS http://127.0.0.1:3456/api/v1/health
```

Repository-local defaults are frontend `19877` and API `3456`; PostgreSQL and Redis remain internal. In the Base umbrella runtime, frontend/API are published at `7722`/`7721`, while PostgreSQL/Redis bind only to loopback `7723`/`7724`. Those are deployment-local coordinates, not public endpoints.

Use [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/API.md](docs/API.md) and [docs/CLI.md](docs/CLI.md) for deployment, contract and CLI detail.

<a name="visual-proof"></a>
## Visual Proof

The root README retains only reviewed seeded evidence. It deliberately excludes the login surface because it advertises MVP demo authentication behavior, and excludes issue/detail views that carry more workflow fixture context than an entry point needs. The full route gallery is available under [docs/screenshots](docs/screenshots).

### Planning evidence

![Task Tracker reports](docs/screenshots/10-reports.png)

### Kanban on mobile

![Task Tracker mobile board](docs/screenshots/m-board-viewport.png)

This is a `375x812` viewport proof. The board intentionally becomes a vertical card stack at mobile width; columns with many cards remain scroll-heavy by design.

<a name="safety"></a>
## Safety Boundaries

- **Deployment secrets.** Compose startup requires a database password and JWT secret. Never commit `.env`, API tokens or real data.
- **Network surface.** PostgreSQL and Redis are internal in repository Compose. The umbrella publishes their debugging ports only on loopback.
- **Auth and ingress.** Central identity integration is configuration-dependent; shared deployment still needs explicit JWT, CORS, cookie, TLS and reverse-proxy review.
- **Metrics exposure.** `/metrics` is configurable so an operator can keep Prometheus scraping on an internal network. It is not an authorization substitute.
- **Health semantics.** `/api/v1/health` is the current liveness endpoint. Do not infer database, email or central-auth provider health solely from a successful liveness response.

Read [docs/SECURITY.md](docs/SECURITY.md) and [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) before any shared deployment.

<a name="quality"></a>
## Quality and Verification

| Gate | Command |
|---|---|
| README contract tests | `python3 -m unittest scripts.tests.test_verify_readme -v` |
| README assets and anchors | `python3 scripts/verify_readme.py` |
| Backend workspace | `cd backend && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -- --test-threads=1` |
| Frontend API/type/test/lint/build | `cd frontend && pnpm openapi:check && pnpm typecheck && pnpm test -- --run && pnpm lint && pnpm format:check && pnpm build` |
| Browser E2E | `cd frontend && pnpm test:e2e -- --project=chromium` |
| Compose contract | `docker compose config -q` |
| Runtime liveness | `curl -fsS http://127.0.0.1:3456/api/v1/health` |

GitHub Actions runs backend formatting/lint/tests, OpenAPI/migrations, coverage, dependency checks, real PostgreSQL tests, frontend gates and browser E2E. The independent README job guards required anchors, reviewed proof, local images, placeholders and accidental local filesystem paths.

## Documentation Map

- **Architecture and scope:** [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/TZ.md](docs/TZ.md), [docs/ROADMAP.md](docs/ROADMAP.md)
- **API and data:** [docs/API.md](docs/API.md), [docs/DATA_MODEL.md](docs/DATA_MODEL.md), [openapi/openapi.json](openapi/openapi.json)
- **Operators:** [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md), [docs/TESTING.md](docs/TESTING.md)
- **Security and code rules:** [docs/SECURITY.md](docs/SECURITY.md), [docs/AGENTS.md](docs/AGENTS.md)

<a name="license"></a>
## License

FerrPOINT Proprietary Source-Available Evaluation License v1.0. This repository is not open source. Viewing and evaluation are allowed under [LICENSE](LICENSE); commercial, production, resale, redistribution and SaaS/hosting use require a written FerrPOINT license. See [NOTICE](NOTICE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
