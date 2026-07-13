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

## 6. Тестовые данные

- Seed scripts: admin user, sample project, workflow, statuses, issue types, 50 задач
- Factory-функции для генерации сущностей
- Snapshot тесты для JQL parser / response DTO
