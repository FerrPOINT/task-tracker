# Архитектура Task Tracker (Jira-like)

## 1. Контекст

Self-hosted таск-трекер, полноценный аналог open-source Jira. Покрывает полный жизненный цикл задач: проекты, типы задач, workflow, kanban/scrum-доски, фильтры, поиск, комментарии, вложения, уведомления, роли/разрешения, спринты, эпики, метрики.

## 2. Структура репозитория (monorepo)

```
task-tracker/
├── backend/
│   ├── crates/
│   │   ├── task-tracker-api/          # HTTP/WS сервер (bin)
│   │   ├── task-tracker-application/  # Сервисы, use-cases, DTO, mappers, политики
│   │   ├── task-tracker-domain/         # Чистые доменные модели, ошибки, traits
│   │   ├── task-tracker-infrastructure/ # Репозитории, клиенты, email, cache, events
│   │   ├── task-tracker-shared/         # Утилиты, ID, время, валидация
│   │   └── task-tracker-cli/            # Admin CLI
│   ├── migrations/
│   ├── tests/
│   └── Cargo.toml
├── frontend/
│   ├── src/
│   │   ├── api/
│   │   ├── features/
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── stores/
│   │   ├── routes/
│   │   ├── i18n/
│   │   ├── types/
│   │   └── utils/
│   ├── tests/
│   └── package.json
├── cli/
├── docs/
│   ├── ARCHITECTURE.md
│   ├── TZ.md
│   ├── PERFORMANCE.md
│   └── TESTING.md
├── docker-compose.yml
├── Dockerfile.backend
├── Dockerfile.frontend
└── README.md
```

## 3. Стек (актуальные версии на 2025–2026)

| Слой | Технология | Версия |
|------|------------|--------|
| Язык backend | Rust | 1.85+ |
| Web-фреймворк | Axum | 0.8+ |
| Runtime | Tokio | 1.43+ |
| База данных | PostgreSQL | 17 |
| Database access | SQLx | 0.8.3+ |
| Миграции | sqlx migrate / refinery | latest |
| Auth | argon2 + jsonwebtoken | latest |
| Валидация | garde | 0.22+ |
| Сериализация | serde + serde_json | 1.0.219+ |
| Логирование | tracing + tracing-subscriber | 0.1.44+ |
| HTTP client | reqwest | 0.12.15+ |
| OpenAPI | utoipa | 5.0+ |
| DI / IoC | shaku или ручной trait-based registry | latest |
| Background jobs | apalis | 0.6+ |
| Event bus | in-memory + Redis pub/sub fallback | — |
| Cache | moka (in-memory) + redis (distributed) | latest |
| Config | figment | 0.10.19+ |
| Metrics | metrics + metrics-exporter-prometheus | latest |
| Email | lettre | 0.11+ |
| Testing | cargo test, mockall, testcontainers, sqlx-testcontainers | latest |
| Frontend | React | 19.1+ |
| Bundler | Vite | 6.2+ |
| TypeScript | TypeScript | 5.9+ |
| Styling | Tailwind CSS | 4.1+ |
| UI kit | shadcn/ui (React 19 + Tailwind v4) | latest |
| Forms | React Hook Form | 7.66+ |
| Валидация форм | Zod | 4.1+ |
| Server state | TanStack Query | 5.93+ |
| Client state | Zustand | 5.0+ |
| Router | React Router | 7.5+ |
| DnD | @dnd-kit/core | 6.3+ |
| i18n | i18next | 25.0+ |
| Markdown | react-markdown | 10.0+ |
| Unit tests | Vitest | 3.2+ |
| E2E / screenshots | Playwright | 1.52+ |
| Component tests | @testing-library/react | 16.3+ |

## 4. Backend архитектура (Rust)

### 4.1 Слоистая архитектура (Spring Boot-like)

```
┌─────────────────────────────────────────────────────────────┐
│  Presentation Layer (API crate)                              │
│  Controller → Request DTO → Mapper → Handler/Extractor       │
├─────────────────────────────────────────────────────────────┤
│  Application Layer (Application crate)                       │
│  Service → Command/Query UseCase → DTO → Mapper → Domain      │
│  ├─ Transaction boundary                                     │
│  ├─ Policy / Permission checks                               │
│  ├─ Event publishing                                          │
│  └─ Notification scheduling                                   │
├─────────────────────────────────────────────────────────────┤
│  Domain Layer (Domain crate)                                   │
│  Entity → Value Object → Aggregate → Domain Event → Repository │
│  Trait (port)                                                 │
├─────────────────────────────────────────────────────────────┤
│  Infrastructure Layer (Infrastructure crate)                   │
│  RepositoryImpl → SQLx Client → PostgreSQL                     │
│  EmailClient → lettre → SMTP                                   │
│  CacheClient → moka / redis                                    │
│  EventBusImpl → broadcast / redis                              │
│  SearchClient → pg_search / meilisearch                        │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 Workspace crates

- `task-tracker-domain` — чистые доменные типы, `Error` enum, `Result`, ID-типы, константы, repository traits, domain events. Не зависит от infrastructure.
- `task-tracker-application` — сервисы, use cases, DTO, mappers, политики, валидация. Зависит от `domain`.
- `task-tracker-infrastructure` — SQLx-репозитории, email, cache, event bus, search, file storage. Зависит от `domain`.
- `task-tracker-api` — Axum-роуты, контроллеры, middleware, WebSocket, DI-реестр, OpenAPI. Зависит от `application` + `infrastructure`.
- `task-tracker-shared` — утилиты: UUID, время, пагинация, пароли, JWT helpers.
- `task-tracker-cli` — admin CLI.

Зависимости направлены внутрь: `api → application → domain ← infrastructure`.

### 4.3 Presentation Layer — Controller → Mapper → DTO

```
backend/crates/task-tracker-api/src/
  controllers/
    auth_controller.rs
    project_controller.rs
    issue_controller.rs
    board_controller.rs
    comment_controller.rs
    filter_controller.rs
    admin_controller.rs
  dto/
    request/
      create_issue_request.rs
      update_issue_request.rs
      move_issue_request.rs
    response/
      issue_response.rs
      board_response.rs
      user_response.rs
  mappers/
    issue_mapper.rs
    board_mapper.rs
    user_mapper.rs
  extractors/
    current_user.rs       # Извлекает UserContext из JWT
    validated_json.rs       # garde + axum extractor
  routes/
    mod.rs
    v1/
      auth.rs
      projects.rs
      issues.rs
      boards.rs
      admin.rs
```

Пример контроллера:

```rust
pub async fn create_issue(
    State(ctx): State<AppContext>,
    CurrentUser(user): CurrentUser,
    ValidatedJson(req): ValidatedJson<CreateIssueRequest>,
) -> ApiResult<Json<IssueResponse>> {
    let cmd = IssueMapper::to_create_command(req, user.id);
    let issue = ctx.issue_service.create(cmd).await?;
    let response = IssueMapper::to_response(issue);
    Ok(Json(response))
}
```

### 4.4 Application Layer — Service → UseCase → Policy

```
backend/crates/task-tracker-application/src/
  services/
    auth_service.rs
    user_service.rs
    project_service.rs
    issue_service.rs
    board_service.rs
    filter_service.rs
    search_service.rs
    comment_service.rs
    attachment_service.rs
    notification_service.rs
    admin_service.rs
    workflow_service.rs
    role_service.rs
  use_cases/
    issues/
      create_issue.rs
      update_issue.rs
      move_issue.rs
      delete_issue.rs
      bulk_update_issues.rs
    projects/
      create_project.rs
      update_project.rs
      add_project_member.rs
  policies/
    issue_policy.rs       # can_update?, can_delete?
    project_policy.rs
    board_policy.rs
  dto/
    issue_dto.rs
    board_dto.rs
  mappers/
    issue_dto_mapper.rs
  events/
    handlers/
      issue_event_handlers.rs
```

Пример сервиса:

```rust
pub struct IssueService {
    issue_repo: Arc<dyn IssueRepository>,
    project_repo: Arc<dyn ProjectRepository>,
    permission_svc: Arc<dyn PermissionService>,
    event_bus: Arc<dyn EventBus>,
    notifier: Arc<dyn NotificationService>,
    tx: Arc<dyn UnitOfWork>,
}

impl IssueService {
    pub async fn create(&self,
        actor: UserContext,
        cmd: CreateIssueCommand,
    ) -> Result<IssueDto> {
        let project = self.project_repo.get_by_id(cmd.project_id).await?;
        self.permission_svc.ensure(&actor, Permission::IssueCreate, &project).await?;

        let issue = self.tx.transaction(|tx| async move {
            let mut issue = Issue::create(cmd, &project)?;
            self.issue_repo.save(tx, &issue).await?;
            self.event_bus.publish(IssueCreatedEvent::from(&issue)).await?;
            Ok::<_, DomainError>(issue)
        }).await?;

        self.notifier.notify_assignee(&issue).await.ok(); // best-effort
        Ok(IssueDto::from(issue))
    }
}
```

### 4.5 Domain Layer — Entity + Repository Trait + Domain Event

```
backend/crates/task-tracker-domain/src/
  entities/
    user.rs
    project.rs
    issue.rs
    issue_status.rs
    workflow.rs
    comment.rs
    attachment.rs
    board.rs
    sprint.rs
    filter.rs
    notification.rs
  value_objects/
    project_key.rs
    issue_number.rs
    email.rs
    password_hash.rs
  events/
    issue_created.rs
    issue_updated.rs
    issue_moved.rs
    comment_added.rs
  repositories/
    user_repository.rs          # trait
    issue_repository.rs
    project_repository.rs
  errors.rs
  id.rs
```

Пример доменного объекта:

```rust
pub struct Issue {
    pub id: IssueId,
    pub project_id: ProjectId,
    pub number: IssueNumber,
    pub issue_type: IssueType,
    pub title: String,
    pub description: Option<String>,
    pub status_id: StatusId,
    pub priority: Priority,
    pub assignee_id: Option<UserId>,
    pub reporter_id: UserId,
    pub labels: Vec<LabelId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Issue {
    pub fn create(cmd: CreateIssueCommand, project: &Project) -> Result<Self> {
        // business invariants
    }

    pub fn transition(&mut self, transition: WorkflowTransition, actor: UserId) -> Result<Vec<DomainEvent>> {
        // проверка условий, генерация событий
    }
}
```

### 4.6 Infrastructure Layer — RepositoryImpl / Client

```
backend/crates/task-tracker-infrastructure/src/
  persistence/
    db.rs
    repositories/
      user_repository_impl.rs
      issue_repository_impl.rs
      project_repository_impl.rs
    models/
      issue_row.rs        # SQLx FromRow
    mappers/
      issue_row_mapper.rs
  cache/
    cache_provider.rs
    moka_provider.rs
    redis_provider.rs
  email/
    email_client.rs
    lettre_client.rs
    templates/
  events/
    event_bus.rs
    memory_event_bus.rs
    redis_event_bus.rs
  search/
    search_client.rs
    postgres_search_client.rs
    meilisearch_client.rs
  storage/
    file_storage.rs
    local_file_storage.rs
    s3_file_storage.rs
  config/
    infrastructure_config.rs
```

Пример репозитория:

```rust
pub struct SqlxIssueRepository {
    pool: PgPool,
}

#[async_trait]
impl IssueRepository for SqlxIssueRepository {
    async fn get_by_id(&self, id: IssueId) -> Result<Option<Issue>> {
        let row = sqlx::query_as!(
            IssueRow,
            r#"SELECT ... FROM issues WHERE id = $1"#,
            id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(IssueRowMapper::to_domain))
    }
}
```

### 4.7 Dependency Injection / IoC (Spring Boot-like)

Rust не имеет runtime reflection, поэтому используем **trait-based DI + manual registry** или `shaku`.

#### Вариант A: ручной AppContext (рекомендуется)

```rust
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub user_service: Arc<dyn UserService>,
    pub issue_service: Arc<dyn IssueService>,
    pub project_service: Arc<dyn ProjectService>,
    pub event_bus: Arc<dyn EventBus>,
    pub ws_hub: Arc<WsHub>,
}

impl AppContext {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let pool = create_pool(&config.database_url).await?;
        let event_bus = Arc::new(MemoryEventBus::new()) as Arc<dyn EventBus>;
        let user_repo = Arc::new(SqlxUserRepository::new(pool.clone())) as Arc<dyn UserRepository>;
        let issue_repo = Arc::new(SqlxIssueRepository::new(pool.clone())) as Arc<dyn IssueRepository>;
        let project_repo = Arc::new(SqlxProjectRepository::new(pool.clone())) as Arc<dyn ProjectRepository>;
        let permission_svc = Arc::new(PermissionServiceImpl::new(project_repo.clone())) as Arc<dyn PermissionService>;
        let user_service = Arc::new(UserServiceImpl::new(user_repo.clone())) as Arc<dyn UserService>;
        let issue_service = Arc::new(IssueServiceImpl::new(issue_repo, project_repo, permission_svc, event_bus.clone())) as Arc<dyn IssueService>;

        Ok(Self { ... })
    }
}
```

Передаётся в Axum через `State`:

```rust
let app = Router::new()
    .nest("/api/v1", v1_routes())
    .with_state(ctx);
```

#### Вариант B: shaku

```rust
module! {
    AppModule: Arc<dyn AppContextTrait> {
        components = [SqlxUserRepository, UserServiceImpl],
        providers = []
    }
}
```

Подходит для больших команд, но добавляет макросы и сложность отладки.

### 4.8 Spring Boot-подобные фишки в Rust

| Spring Boot | Rust-аналог |
|-------------|-------------|
| `@ConfigurationProperties` | `figment` + `serde` |
| `@Component` / `@Service` | ручной `Arc<dyn Trait>` registry или `shaku` |
| `@Autowired` | конструкторный DI |
| `@RestController` | Axum router + handler fn |
| `@RequestMapping` | `Router::nest`, `get`, `post` |
| `@Valid` | garde extractor `ValidatedJson` |
| `@Transactional` | `UnitOfWork` wrapper / SQLx `transaction` |
| JPA Repository | SQLx + repository trait + impl |
| `@EventListener` | `EventBus::subscribe` |
| `@Scheduled` / `@Async` | `apalis` + `tokio::spawn` |
| Spring Cache | `moka` / `redis` |
| Spring Mail | `lettre` |
| Spring Security | custom Axum middleware + PermissionService |
| Spring Data JPA Specifications | JQL-подобный AST → SQL builder |
| Actuator / Metrics | `metrics` + `metrics-exporter-prometheus` |
| AOP logging | `tracing` + Tower trace layer |
| `@Profile` | feature flags + config |

### 4.9 Config (Spring Boot-like `@ConfigurationProperties`)

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub http_port: u16,
    pub jwt_secret: String,
    pub jwt_access_ttl: u64,
    pub jwt_refresh_ttl: u64,
    pub log_level: String,
    pub redis_url: Option<String>,
    pub smtp: SmtpConfig,
}

pub fn load_config() -> Result<AppConfig> {
    Figment::new()
        .merge(Toml::file("config.toml"))
        .merge(Env::prefixed("TASKTRACKER_"))
        .extract()
        .map_err(|e| e.into())
}
```

### 4.10 Middleware (Spring Boot-like interceptors)

```rust
let app = Router::new()
    .layer(TraceLayer::new_for_http())
    .layer(CorsLayer::new().allow_origin(AllowOrigin::exact(config.frontend_url)))
    .layer(GovernorLayer {
        config: &rate_limit_config,
    })
    .layer(RequestIdLayer)
    .layer(CompressionLayer::new());
```

Кастомные middleware:

- `AuthMiddleware` — извлечение JWT.
- `PermissionMiddleware` — проверка глобальных прав.
- `RateLimitMiddleware` — `tower-governor`.
- `TraceMiddleware` — `tracing` span с request_id.
- `ErrorMappingMiddleware` — единый формат ошибок.

### 4.11 Auth

- Регистрация: email + пароль.
- Пароль: argon2id (memory=19 MiB, iterations=2, parallelism=1).
- Access token: JWT, TTL 15 мин.
- Refresh token: UUIDv4, хранится в PostgreSQL `refresh_tokens`, передаётся в `httpOnly` cookie.
- Middleware извлекает access token из `Authorization: Bearer ...` или cookie.
- CSRF: SameSite=Lax для cookie + CORS origin whitelist.

### 4.12 Роли и разрешения

- Глобальные роли: `system_admin`, `user`, `guest`.
- Проектные роли: `project_admin`, `project_lead`, `developer`, `viewer`.
- Разрешения: `issue:create`, `issue:update`, `issue:delete`, `project:settings`, `board:admin`, `filter:manage`, `user:admin`.
- Проверка в `PermissionService` на уровне application.

### 4.13 Workflow

- Каждый проект имеет набор статусов.
- Статус привязан к категории: `todo`, `in_progress`, `done`.
- Workflow-переходы задают разрешённые переходы и проверки.
- Перемещение задачи на доске = `POST /issues/:id/transitions`.

### 4.14 Realtime

- WebSocket endpoint `/ws`.
- Аутентификация: access token в query-параметре `?token=`.
- Каналы: `user:{id}`, `project:{id}`, `issue:{id}`, `board:{project_id}`.
- Fallback: Server-Sent Events `/sse`.
- События: `IssueCreated`, `IssueUpdated`, `IssueMoved`, `CommentAdded`, `Notification`, `BoardRefresh`.

### 4.15 Background jobs / scheduling

- `apalis` — type-safe job queue на PostgreSQL или Redis.
- Типы задач:
  - `SendEmailJob`
  - `GenerateThumbnailJob`
  - `IndexSearchJob`
  - `CleanupOldExportsJob`
- CRON через `apalis::cron`.

### 4.16 Event bus

- In-memory: `tokio::sync::broadcast`.
- Distributed: Redis pub/sub.
- События: domain events → application handlers → notifications/realtime/cache invalidation.

### 4.17 Cache

- `moka` — in-memory cache для справочников и permissions.
- `redis` — distributed cache, rate limit, WS pub/sub.
- TTL: справочники — 5 мин, permissions — 1 мин, dashboard — 30 сек.
- Invalidation через event bus.

### 4.18 Email

- `lettre` + SMTP.
- Шаблоны: `handlebars` или `tera`.
- Очередь через `apalis`.
- Fallback: сохранение в `email_queue` для retry.

### 4.19 Metrics / Observability

- `tracing` structured JSON logs.
- OpenTelemetry traces.
- Prometheus metrics:
  - `http_requests_total`
  - `http_request_duration_seconds`
  - `db_pool_active_connections`
  - `ws_connections_total`
  - `jobs_processed_total`
- Health endpoint `/health`, readiness `/ready`, metrics `/metrics`.

### 4.20 Search

- PostgreSQL `tsvector` для полнотекста.
- JQL-подобный язык → AST → SQL builder.
- Альтернатива: `meilisearch` для больших инсталляций.

## 5. Frontend архитектура (React 19)

### 5.1 Структура `frontend/src`

```
src/
  api/                   # Клиент axios/fetch, TanStack Query hooks
    client.ts
    hooks/
      useAuth.ts
      useProjects.ts
      useIssues.ts
      useBoard.ts
      useFilters.ts
      useNotifications.ts
  features/              # Feature-based модули
    auth/
    projects/
    issues/
    board/
    filters/
    search/
    comments/
    attachments/
    admin/
    notifications/
    settings/
  components/            # Переиспользуемые UI-компоненты
    ui/                  # shadcn/ui база
    layout/
    data-table/
    board/
    forms/
  hooks/                 # Общие hooks
  stores/                # Zustand
    authStore.ts
    uiStore.ts
    boardStore.ts
  routes/                # React Router 7
  i18n/                  # ru, en
  types/                 # openapi-typescript
  utils/
  main.tsx
  App.tsx
```

### 5.2 Принципы

- Feature-based структура.
- Server state — TanStack Query.
- Client state — Zustand.
- Forms — React Hook Form + Zod.
- Routing — React Router 7 data-mode.
- Тёмная тема по умолчанию.

### 5.3 Доска

- `@dnd-kit/core` + `@dnd-kit/sortable`.
- Optimistic update в TanStack Query.
- Realtime: при `BoardRefresh` — refetch.

### 5.4 Поиск и фильтры

- JQL-подобный язык.
- Сохранённые фильтры.
- Полнотекстовый поиск.

## 6. База данных

### 6.1 Таблицы

- `users`, `user_profiles`, `user_settings`
- `roles`, `permissions`, `role_permissions`, `project_roles`, `project_members`
- `projects`, `project_keys`
- `issue_types`, `issue_types_in_projects`
- `issue_statuses`, `status_categories`, `workflow_transitions`
- `issues`, `issue_history`, `issue_links`, `issue_relations`
- `comments`
- `attachments`
- `labels`, `issue_labels`
- `components`, `issue_components`
- `versions`, `issue_fix_versions`, `issue_affected_versions`
- `boards`, `board_columns`, `board_column_issues`
- `sprints`, `sprint_issues`
- `filters`, `filter_queries`
- `notifications`
- `email_queue`
- `audit_log`
- `refresh_tokens`
- `api_keys`
- `webhooks`

### 6.2 Индексы

- B-tree: `issues.project_id`, `issues.status_id`, `issues.assignee_id`, `issues.created_at`.
- GIN: `issues.search_vector`, `issues.custom_fields`.
- Partial: `notifications(user_id, is_read) WHERE is_read = false`.

## 7. API

### 7.1 REST

- `/api/v1/...`.
- Ошибки:
  ```json
  {
    "error": {
      "code": "VALIDATION_ERROR",
      "message": "...",
      "details": {}
    }
  }
  ```
- Пагинация: cursor-based для лент, offset-based для таблиц.

### 7.2 OpenAPI

- `utoipa` генерирует спецификацию.
- Scalar UI по `/api/docs`.
- `openapi-typescript` генерирует frontend-типы.

## 8. Тестирование

Подробнее в `TESTING.md`.

## 9. Инфраструктура

### 9.1 Docker Compose

- PostgreSQL 17.
- Backend port 19876.
- Frontend dev port 19877.
- Production: nginx + backend.

### 9.2 ENV

Префикс `TASKTRACKER_`:

- `TASKTRACKER_DATABASE_URL`
- `TASKTRACKER_HTTP_PORT=19876`
- `TASKTRACKER_JWT_SECRET`
- `TASKTRACKER_REDIS_URL`
- `TASKTRACKER_SMTP_HOST`, `TASKTRACKER_SMTP_FROM`

## 10. Безопасность

- Argon2id, JWT, httpOnly refresh cookie.
- Rate limiting, CORS whitelist.
- Input validation (garde + Zod).
- XSS/CSRF защита.
- File upload sandbox.

## 11. Производительность

Подробнее в `PERFORMANCE.md`.

## 12. Эволюция

1. MVP-каркас.
2. Core Jira.
3. Agile.
4. Enterprise.

## 13. Референс

- Atlassian Jira Data Center / Server.
- OpenProject.
- YouTrack.
- Redmine.
