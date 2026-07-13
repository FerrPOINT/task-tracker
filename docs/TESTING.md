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
- `playwright` 1.51.1
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

- GitHub Actions
- Pipeline:
  1. `cargo fmt --check`, `cargo clippy -- -D warnings`
  2. `cargo test`
  3. `pnpm lint`, `pnpm typecheck`, `vitest run`
  4. `playwright test`
  5. Docker build
  6. Push image

## 5. Чек-лист перед merge

- [ ] Все unit-тесты проходят
- [ ] Интеграционные тесты проходят
- [ ] E2E critical path проходит
- [ ] Скриншоты для трёх разрешений приложены
- [ ] Performance budget не нарушен
- [ ] OpenAPI spec не сломан

## 6. Fixtures and Factories

### 6.1 Backend Fixtures

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

### 6.2 Factory Example (Rust)

```rust
pub fn issue_factory() -> IssueFactory {
    IssueFactory::default()
        .project_id(Uuid::new_v4())
        .issue_type_id(Uuid::new_v4())
        .summary("Test issue".to_string())
}
```

### 6.3 Frontend Fixtures

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

### 6.4 Seed Scripts

- `seed_dev.ts` — admin, sample project, workflow, statuses, issue types, 50 задач.
- `seed_e2e.ts` — deterministic dataset для Playwright.

## 7. Coverage

### 7.1 Targets

| Layer | Target |
|-------|--------|
| Domain / services | >= 80% |
| Repository | >= 70% |
| API controllers | >= 75% |
| Frontend utilities / hooks | >= 70% |
| Critical UI components | >= 60% |
| E2E critical path | 100% |

### 7.2 Tools

- Backend: `cargo tarpaulin` (или `cargo-llvm-cov`).
- Frontend: `vitest --coverage` + `@vitest/coverage-v8`.
- CI gate: coverage не должно падать.

### 7.3 Exclusions

- Generated code (DTOs from openapi-generator).
- UI mockups.
- Third-party vendored code.
- Pure type definitions.

## 8. Snapshot Tests

- JQL parser AST snapshots.
- API response DTO snapshots.
- Frontend component snapshots для стабильных UI-элементов.

## 9. Contract Tests

- Pact / openapi-validator для backend-frontend API contract.
- Run against generated `openapi.json`.

## 10. Security Tests

- OWASP ZAP scan.
- `cargo audit` / `pnpm audit`.
- Rate limit tests.
- Auth/permission edge cases.
## References

- `docs/ARCHITECTURE.md`
- `docs/CODE_STYLE.md`
- `docs/DEPLOYMENT.md`
