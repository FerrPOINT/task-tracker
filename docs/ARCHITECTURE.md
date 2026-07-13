# Архитектура Task Tracker (Jira-like)

## 1. Контекст

Self-hosted таск-трекер, полноценный аналог open-source Jira. Покрывает полный жизненный цикл задач: проекты, типы задач, workflow, kanban/scrum-доски, эпики, спринты, фильтры, поиск (JQL), комментарии, вложения, уведомления, роли и разрешения.

## 2. Технологический стек (актуальные версии)

### Backend
- **Rust**: 1.88.0+ (latest stable на июль 2026)
- **Web framework**: `axum` 0.8.9
- **Async runtime**: `tokio` 1.52.3 (`full`)
- **DB access**: `sea-orm` 2.0.x + `sqlx` 0.9.0
- **Migrations**: `refinery` 0.8.15 (или SeaORM Migrator)
- **Config**: `figment` 0.10.19
- **Validation**: `garde` 0.23.0
- **Auth**: `argon2` 0.6.0-pre.1, `jsonwebtoken` 10.4.0
- **Email**: `lettre` 0.11.22
- **Queue/scheduler**: `apalis` 0.7.4
- **Cache**: `moka` 0.12.15
- **Redis**: `redis` 1.3.0
- **HTTP client**: `reqwest` 0.13.4
- **Metrics**: `metrics` 0.24.6 + `metrics-exporter-prometheus`
- **Tracing**: `tracing` 0.1.44
- **OpenAPI**: `utoipa` 5.5.0
- **DI**: ручной `AppContext` (`Arc<dyn Trait>`); опционально `shaku` 0.6.2
- **Testing**: `mockall` 0.15.0, `testcontainers` 0.27.3
- **Rate limiting**: `tower_governor` 0.8.0
- **HTTP middleware**: `tower-http` 0.7.0

### Frontend
- **React**: 19.1.0
- **Build**: `vite` 6.2.0
- **TypeScript**: 5.9.3
- **Styling**: `tailwindcss` 4.1.0, `@tailwindcss/vite` 4.1.0
- **Components**: `shadcn/ui` (React 19 compatible)
- **State**: `zustand` 5.0.3
- **Query**: `@tanstack/react-query` 5.74.4
- **Router**: `react-router` 8.1.0
- **Forms**: `react-hook-form` 7.55.0 + `zod` 4.4.3
- **Utils**: `@tanstack/react-table`, `date-fns` 4.1.0, `@dnd-kit/core`, `@dnd-kit/sortable`, `@tiptap/react`, `sonner`, `@tanstack/react-virtual`
- **Testing**: `vitest` 4.1.10, `@testing-library/react` 16.3.0, `playwright` 1.61.1

### Infrastructure
- **БД**: PostgreSQL 17.6
- **Cache/queue**: Redis 8.0 (Valkey 8.1 как fallback)
- **Reverse proxy / load balancer**: Traefik 3.4
- **Container runtime**: Docker + Docker Compose

## 3. Структура монорепозитория

```
task-tracker/
├── backend/
│   ├── Cargo.toml
│   ├── crates/
│   │   ├── api/
│   │   ├── app/
│   │   ├── domain/
│   │   ├── infra/
│   │   ├── shared/
│   │   └── server/
│   └── migrations/
├── frontend/
│   ├── src/
│   │   ├── api/
│   │   ├── app/
│   │   ├── entities/
│   │   ├── features/
│   │   ├── shared/
│   │   └── widgets/
│   ├── playwright/
│   └── vitest/
├── cli/
│   └── src/
└── docs/
    ├── ADR.md
    ├── AGENTS.md
    ├── API.md
    ├── API_EDGE_CASES.md
    ├── API_VERSIONING.md
    ├── ARCHITECTURE.md
    ├── AUTH_ADVANCED.md
    ├── CACHING.md
    ├── CI_CD.md
    ├── CLI.md
    ├── CODE_STYLE.md
    ├── DATABASE_INDEXES.md
    ├── DATA_MODEL.md
    ├── DATA_RETENTION.md
    ├── DEPLOYMENT.md
    ├── DESIGN_TOKENS.md
    ├── ERROR_HANDLING.md
    ├── EVENTS.md
    ├── FEATURE_FLAGS.md
    ├── FRONTEND_ARCHITECTURE.md
    ├── GLOSSARY.md
    ├── I18N.md
    ├── JIRA_GAP_DETAILS.md
    ├── JIRA_UI_CAPTURE.md
    ├── JQL.md
    ├── LIBRARIES.md
    ├── LOAD_BALANCING.md
    ├── MIGRATIONS.md
    ├── MONITORING.md
    ├── NOTIFICATIONS.md
    ├── ONBOARDING.md
    ├── OPS_RUNBOOK.md
    ├── PAGINATION.md
    ├── PERFORMANCE.md
    ├── PROJECT_ADMIN.md
    ├── REACT_STYLING.md
    ├── RELEASE.md
    ├── REPORTS.md
    ├── RESILIENCE.md
    ├── ROADMAP.md
    ├── ROUTING.md
    ├── RUNTIME.md
    ├── SECURITY.md
    ├── SECURITY_INCIDENT_RESPONSE.md
    ├── STORAGE.md
    ├── SYSTEM_ADMIN.md
    ├── TESTING.md
    ├── TZ.md
    ├── UI_LIBRARIES.md
    ├── UI_UX.md
    ├── USER_STORIES.md
    ├── UX_PRODUCT.md
    ├── VIKUNJA_GAP_ANALYSIS.md
    ├── WEBSOCKET_EVENTS.md
    ├── WORKFLOW.md
    └── adr/
├── frontend/
│   ├── src/
│   │   ├── api/
│   │   ├── app/
│   │   ├── entities/
│   │   ├── features/
│   │   ├── shared/
│   │   └── widgets/
│   ├── playwright/
│   └── vitest/
├── cli/
│   └── src/
└── docs/
    ├── ADR.md
    ├── AGENTS.md
    ├── API.md
    ├── API_VERSIONING.md
    ├── ARCHITECTURE.md
    ├── CACHING.md
    ├── CLI.md
    ├── CODE_STYLE.md
    ├── DATABASE_INDEXES.md
    ├── DATA_MODEL.md
    ├── DEPLOYMENT.md
    ├── DESIGN_TOKENS.md
    ├── ERROR_HANDLING.md
    ├── EVENTS.md
    ├── FRONTEND_ARCHITECTURE.md
    ├── GLOSSARY.md
    ├── I18N.md
    ├── JIRA_UI_CAPTURE.md
    ├── JQL.md
    ├── LIBRARIES.md
    ├── MIGRATIONS.md
    ├── MONITORING.md
    ├── NOTIFICATIONS.md
    ├── ONBOARDING.md
    ├── OPS_RUNBOOK.md
    ├── PERFORMANCE.md
    ├── PROJECT_ADMIN.md
    ├── REACT_STYLING.md
    ├── RELEASE.md
    ├── REPORTS.md
    ├── ROADMAP.md
    ├── ROUTING.md
    ├── SECURITY.md
    ├── STORAGE.md
    ├── SYSTEM_ADMIN.md
    ├── TESTING.md
    ├── TZ.md
    ├── UI_LIBRARIES.md
    ├── UI_UX.md
    ├── USER_STORIES.md
    ├── VIKUNJA_GAP_ANALYSIS.md
    ├── WEBSOCKET_EVENTS.md
    ├── WORKFLOW.md
    ├── adr/
    └── assets/
        ├── ui-mockups/
        │   ├── issue-detail.html
        │   ├── kanban-board.html
        │   └── project-list.html
        └── jira-samples/
            ├── issue-shape.json
            ├── custom-field-shape.json
            └── board-config-shape.json
```

## 4. Backend: чёткие слои

### 4.1 Presentation layer (`crates/api`)

**Controller** — тонкий HTTP-адаптер. Только:
- extract path/query/body/headers/auth state
- вызов `Service::handle(command).await`
- map `ServiceResult<T>` → `Response<T>`

```rust
pub async fn create_issue(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateIssueRequest>,
) -> Result<Json<IssueResponse>, ApiError> {
    let cmd = CreateIssueCommand::try_from(req)?;
    let issue = ctx.issue_service.create(cmd).await?;
    Ok(Json(IssueResponse::from(issue)))
}
```

**DTO**:
- `*Request` — inbound JSON с `garde` derive-валидацией
- `*Response` — outbound JSON
- `*Command` / `*Query` — application input (без HTTP)
- Маппинг Request → Command, Entity → Response — честные `From`/`TryFrom`.

### 4.2 Application layer (`crates/app`)

**Service** — единица бизнес-операции. Содержит:
- авторизацию через `PermissionService`
- валидацию бизнес-правил
- координацию репозиториев
- публикацию доменных событий
- отправку уведомлений / задач в очередь

```rust
impl IssueService {
    pub async fn create(&self, cmd: CreateIssueCommand) -> Result<IssueDto, AppError> {
        self.authz.ensure(&cmd.project_id, Permission::CreateIssue).await?;
        let issue = self.uow.with_transaction(|repos| async move {
            let project = repos.projects.get(cmd.project_id).await?;
            let issue = Issue::create(project, cmd.fields)?;
            repos.issues.save(&issue).await?;
            repos.events.publish(issue.events()).await?;
            Ok(issue)
        }).await?;
        self.notifier.notify_subscribers(&issue).await;
        Ok(IssueDto::from(issue))
    }
}
```

**Mapper/DTO**: каждая сущность имеет `Dto` и `Response`. Маппинг не выполняет бизнес-логику.

### 4.3 Domain layer (`crates/domain`)

**Entity**:
```rust
pub struct Issue {
    pub id: IssueId,
    pub project_id: ProjectId,
    pub key: IssueKey,
    pub status: StatusId,
    pub summary: String,
    pub description: Option<RichText>,
    pub assignee: Option<UserId>,
    pub reporter: UserId,
    pub priority: Priority,
    pub labels: Vec<LabelId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub domain_events: Vec<IssueEvent>,
}
```

**Value Objects**: `IssueKey`, `ProjectKey`, `RichText`, `JqlQuery`, `WorkflowTransition`.

**Domain Events**:
```rust
pub enum IssueEvent {
    Created { issue_id: IssueId, reporter: UserId },
    StatusChanged { issue_id: IssueId, from: StatusId, to: StatusId },
    Assigned { issue_id: IssueId, assignee: Option<UserId> },
}
```

**Repository traits**:
```rust
#[async_trait]
pub trait IssueRepository: Send + Sync {
    async fn get(&self, id: IssueId) -> Result<Issue, DomainError>;
    async fn save(&self, issue: &Issue) -> Result<(), DomainError>;
    async fn list(&self, query: IssueQuery) -> Result<Paginated<Issue>, DomainError>;
}
```

### 4.4 Infrastructure layer (`crates/infra`)

**SQLx/SeaORM repositories** — реализации repository trait. SeaORM для стандартных CRUD, sqlx для сложных JQL-запросов и отчётов.

**Unit of Work** — транзакционная обёртка:
```rust
pub trait UnitOfWork: Send + Sync {
    async fn with_transaction<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'a> FnOnce(&'a Repositories) -> BoxFuture<'a, Result<T, E>>,
        E: From<InfraError>;
}
```

**Outbound clients**:
- `EmailClient` (lettre)
- `SearchClient` (meilisearch/opensearch как опция)
- `WebhookClient` (reqwest)
- `FileStore` (S3-compatible / filesystem)

**Event bus**:
- in-memory broadcast для single-instance
- redis pub/sub для multi-instance

## 5. Spring Boot-аналоги в Rust

| Spring Boot | Rust эквивалент |
|---|---|
| `@ConfigurationProperties` | `figment` (TOML/JSON/ENV merge) + `serde` struct |
| `@Service` / `@Component` | trait + `Arc<dyn Trait>` в `AppContext` |
| `@Autowired` / constructor DI | ручной `new(...)` в `server::build_context()`; либо `shaku` |
| `@Transactional` | `UnitOfWork::with_transaction` |
| `@Scheduled` | `apalis` cron worker |
| Spring Cache | `moka` in-memory; `redis` distributed |
| Spring Mail | `lettre` через `apalis` queue |
| Spring Security | axum middleware + `PermissionService` |
| Spring Boot Actuator | `metrics` + `/health`, `/metrics`, `/ready` |
| Spring Data JPA | `sqlx` + ручные repository traits |
| Spring Validation | `garde` derive |
| Spring Web / Controller | `axum` handlers |
| Spring Cloud Config | `figment` env override |
| Spring Session | `redis` + JWT refresh cookie |

## 6. Конфигурация

```toml
# backend/crates/shared/src/config.rs
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub smtp: Option<SmtpConfig>,
    pub search: SearchConfig,
}
```

Загрузка:
```rust
let config: AppConfig = Figment::new()
    .merge(Toml::file("config.toml"))
    .merge(Env::prefixed("TASKTRACKER_"))
    .extract()?;
```

## 7. Middleware stack

```rust
let app = Router::new()
    .merge(api_routes())
    .layer(TraceLayer::new_for_http())
    .layer(CompressionLayer::new())
    .layer(CorsLayer::permissive()) // прод production: strict origins
    .layer(GovernorLayer { config: rate_limit_config })
    .layer(TimeoutLayer::new(Duration::from_secs(30)))
    .layer(CatchPanicLayer::new())
    .layer(PropagateRequestIdLayer::new(x_request_id()));
```

## 8. Security

- **AuthN**: JWT access token (15 min) + httpOnly refresh cookie (7 дней)
- **AuthZ**: permission matrix + project roles
- **Hashing**: argon2id
- **Input validation**: `garde` на Request DTO
- **Rate limiting**: `tower_governor` per user / per IP
- **CORS**: строгий whitelist
- **CSP**: через reverse proxy

Политика безопасности детально — `docs/SECURITY.md`.

## 9. Logging, Observability, Scalability

### 9.1 Logging

- JSON structured logs.
- Request ID correlation.
- Levels: ERROR, WARN, INFO, DEBUG, TRACE.
- No sensitive data in logs.
- Local: pretty output. Production: JSON to stdout → Loki.

### 9.2 Observability

- Prometheus metrics: `http_requests_total`, `http_request_duration_seconds`, `db_pool_connections`, `cache_hit_total`, etc.
- Grafana dashboards: API, DB, Cache, Infrastructure, Business.
- OpenTelemetry traces.
- Loki log aggregation.
- Alertmanager rules.

Подробнее — `docs/MONITORING.md`.

### 9.3 Scalability

- API instances are stateless.
- Horizontal scaling via container orchestration.
- WebSocket multi-instance sync via Redis pub/sub.
- DB read replicas for heavy JQL/reports.
- Cache offloading via Redis.
- Async processing via `apalis` workers.

## 10. API, Workflow, JQL, UI, Deployment, and Operations

- REST API specification — `docs/API.md`.
- OpenAPI generation — `utoipa-axum`.
- WebSocket — live updates kanban / issue page, payloads в `docs/API.md`.
- Real-time — redis pub/sub + WS broadcast.
- Technical specification (TZ) — `docs/TZ.md`.
- Data model — `docs/DATA_MODEL.md`.
- Performance goals — `docs/PERFORMANCE.md`.
- Libraries overview — `docs/LIBRARIES.md`.
- Jira UI capture notes — `docs/JIRA_UI_CAPTURE.md`.
- Vikunja gap analysis — `docs/VIKUNJA_GAP_ANALYSIS.md`.
- Workflow engine — `docs/WORKFLOW.md`.
- JQL grammar — `docs/JQL.md`.
- User stories — `docs/USER_STORIES.md`.
- UI/UX specification — `docs/UI_UX.md`.
- Frontend architecture — `docs/FRONTEND_ARCHITECTURE.md`.
- Design tokens — `docs/DESIGN_TOKENS.md`.
- React styling guide — `docs/REACT_STYLING.md`.
- Frontend libraries — `docs/UI_LIBRARIES.md`.
- Project administration — `docs/PROJECT_ADMIN.md`.
- System administration — `docs/SYSTEM_ADMIN.md`.
- Notifications — `docs/NOTIFICATIONS.md`.
- Reports — `docs/REPORTS.md`.
- CLI specification — `docs/CLI.md`.
- AGENTS.md rules — `docs/AGENTS.md`.
- Deployment and Docker Compose — `docs/DEPLOYMENT.md`.
- Database migrations — `docs/MIGRATIONS.md`.
- File storage — `docs/STORAGE.md`.
- Caching strategy — `docs/CACHING.md`.
- Routing — `docs/ROUTING.md`.
- Error handling — `docs/ERROR_HANDLING.md`.
- i18n — `docs/I18N.md`.
- Code style — `docs/CODE_STYLE.md`.
- ADR — `docs/ADR.md`.
- Security — `docs/SECURITY.md`.
- Monitoring — `docs/MONITORING.md`.
- Roadmap — `docs/ROADMAP.md`.
- Database indexes — `docs/DATABASE_INDEXES.md`.
- Glossary — `docs/GLOSSARY.md`.
- API versioning — `docs/API_VERSIONING.md`.
- WebSocket events — `docs/WEBSOCKET_EVENTS.md`.
- Event catalog — `docs/EVENTS.md`.
- Operations runbook — `docs/OPS_RUNBOOK.md`.
- Release process — `docs/RELEASE.md`.
- Onboarding — `docs/ONBOARDING.md`.
- UI mockups — `docs/assets/ui-mockups/`.
- Jira structural samples — `docs/assets/jira-samples/`.

## 10. Testing

Подробнее в `docs/TESTING.md`.

## 11. Deployment

Подробнее в `docs/DEPLOYMENT.md`.

## 12. Security

Подробнее в `docs/SECURITY.md`.

## 13. Monitoring

Подробнее в `docs/MONITORING.md`.

## References

- `README.md`
- `docs/TZ.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md` (корень репозитория)
