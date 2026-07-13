# Стратегия тестирования Task Tracker

## 1. Принципы

- Тесты пишутся одновременно с кодом (TDD для сервисов).
- Каждая доработка UI сопровождается скриншотами.
- Critical path покрыт e2e.
- Регрессионные скриншоты перед merge.

## 2. Backend тесты

### Unit-тесты
- Domain entities: бизнес-правила, workflow transitions
- Mappers: Request → Command → DTO → Response
- Validators: `garde` rules

### Integration-тесты
- Repository tests через `testcontainers` PostgreSQL
- Service tests с in-memory event bus и mock email client
- API tests через `reqwest` к запущенному `TestServer`

### Load-тесты
- `k6`/`oha` на критических эндпоинтах
- Целевые показатели: P95 < 200 мс, 100 RPS

## 3. Frontend тесты

### Unit-тесты
- `vitest` для pure functions, hooks, stores
- `@testing-library/react` для компонентов
- MSW для мока API

### E2E
- `playwright` 1.61.1
- Critical path:
  - регистрация / логин
  - создание проекта
  - создание задачи
  - перемещение на kanban
  - добавление комментария
  - поиск JQL
  - фильтры
- Скриншоты после каждого шага в трёх разрешениях:
  - 375×667 (mobile)
  - 1920×1080 (Full HD)
  - 2560×1440 (2K)

### Visual regression
- Playwright screenshot assertions
- Percy / Chromatic как опция

## 4. CI/CD

Краткий pipeline — детали в `docs/CI_CD.md`:

- GitHub Actions.
- Lint, unit, integration, E2E, coverage, security audit, Docker build.
- Coverage gates block merge.

## 5. Test Isolation

### 5.1 Backend

- Каждый integration test получает свежую логическую БД в одном PostgreSQL контейнере.
- `testcontainers` поднимает Postgres/Redis один раз на suite.
- Миграции применяются в `setup` hook.
- Тесты внутри транзакции с откатом (`BEGIN ... ROLLBACK`).
- Unit тесты параллельны; integration — последовательны внутри suite.

### 5.2 Frontend

- MSW мокает API.
- LocalStorage/SessionStorage мокаются в `setupFiles`.
- Zustand store сбрасывается в `beforeEach`.
- TanStack Query cache — `queryClient.clear()`.

### 5.3 E2E

- Новый браузерный контекст на тест.
- Seed БД через `/api/v1/test/seed`.
- Deterministic fixtures.
- Parallel workers = 4.

## 6. Flaky Tests Strategy

| Problem | Solution |
|---------|----------|
| Async race | `await expect(...).toPass({ timeout: 5000 })` |
| DB ordering | deterministic `ORDER BY` по `created_at` |
| Time-based | fake timers / `timekeeper` |
| Network mocks | strict MSW matching |
| Unstable selectors | `data-testid` |
| Quarantine | flaky test → `@flaky` tag → отдельный CI job с retry |
| Retry policy | Playwright retry=2, unit/integration retry=0 |

## 7. Mutation Testing

- Backend: `cargo-mutation-testing` / `mutagen` для domain/services.
- Frontend: `stryker-js` для критичных pure functions.
- Запускается раз в неделю scheduled job, не блокирует PR.
- Target: mutation score >= 60%.

## 8. Test Artifacts

| Artifact | Path | Retention |
|----------|------|-----------|
| Coverage HTML | `target/tarpaulin/` / `coverage/` | 30 days |
| Playwright report | `playwright-report/` | 14 days |
| Playwright screenshots | `test-results/` | 14 days |
| JUnit XML | `junit-*.xml` | 30 days |
| OpenAPI diff | `openapi-diff.md` | 30 days |

## 9. Чек-лист перед merge

- [ ] Все unit-тесты проходят
- [ ] Интеграционные тесты проходят
- [ ] E2E critical path проходит
- [ ] Скриншоты для трёх разрешений приложены
- [ ] Performance budget не нарушен
- [ ] OpenAPI spec не сломан
- [ ] Coverage gates green

## 10. Fixtures and Factories

### 10.1 Backend Fixtures

```
backend/fixtures/
├── minimal.sql          -- bare minimum for tests
├── dev.sql              -- rich dev dataset
├── e2e.sql              -- deterministic e2e dataset
└── factories/
    ├── users.rs
    ├── projects.rs
    ├── issues.rs
    ├── comments.rs
    └── worklogs.rs
```

### 10.2 Factory Example (Rust)

```rust
pub fn issue_factory() -> IssueFactory {
    IssueFactory::default()
        .project_id(Uuid::new_v4())
        .issue_type_id(Uuid::new_v4())
        .summary("Test issue".to_string())
}
```

### 10.3 Frontend Fixtures

```ts
// test/fixtures/issues.ts
export const issueFixture = (override?: Partial<Issue>): Issue => ({
  id: crypto.randomUUID(),
  key: "PROJ-1",
  summary: "Test issue",
  status: statusFixture(),
  ...override,
})
```

### 10.4 Seed Scripts

- `seed_dev.ts` — admin, sample project, workflow, statuses, issue types, 50 задач.
- `seed_e2e.ts` — deterministic dataset для Playwright.

## 11. Coverage

### 11.1 Targets

| Layer | Target |
|-------|--------|
| Domain / services | >= 80% |
| Repository | >= 70% |
| API controllers | >= 75% |
| Frontend utilities / hooks | >= 70% |
| Critical UI components | >= 60% |
| E2E critical path | 100% |
| Project total | >= 75% |

### 11.2 Tools

- Backend: `cargo tarpaulin` (или `cargo-llvm-cov`).
- Frontend: `vitest --coverage` + `@vitest/coverage-v8`.
- CI gate: coverage не должно падать.

### 11.3 Exclusions

- Generated code (DTOs from openapi-generator).
- UI mockups.
- Third-party vendored code.
- Pure type definitions.

## 12. Snapshot Tests

- JQL parser AST snapshots.
- API response DTO snapshots.
- Frontend component snapshots для стабильных UI-элементов.

## 13. Contract Tests

- Pact / openapi-validator для backend-frontend API contract.
- Run against generated `openapi.json`.

## 14. Security Tests

- OWASP ZAP scan.
- `cargo audit` / `pnpm audit`.
- Rate limit tests.
- Auth/permission edge cases.
## References

- `docs/ARCHITECTURE.md`
- `docs/CI_CD.md`
- `docs/CODE_STYLE.md`
- `docs/DEPLOYMENT.md`
