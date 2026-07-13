# Стратегия тестирования Task Tracker

## 1. Принципы

- Тесты пишутся одновременно с кодом (TDD для сервисов).
- Каждая доработка UI сопровождается скриншотами.
- Critical path покрыт e2e.
- Регрессия отлавливается CI.

## 2. Backend тесты

### 2.1 Unit-тесты

- **Цель**: бизнес-логика сервисов, валидация, workflow, permissions.
- **Фреймворк**: `cargo test`.
- **Подход**: mock-репозитории через trait-объекты.
- **Покрытие**: ≥ 80% для `task-tracker-services` и `task-tracker-auth`.

Пример:

```rust
#[tokio::test]
async fn test_create_issue_assigns_number() {
    let repo = MockIssueRepo::new();
    let svc = IssueService::new(Arc::new(repo), ...);
    let issue = svc.create(ctx, create_cmd).await.unwrap();
    assert_eq!(issue.project_key, "PROJ");
    assert_eq!(issue.number, 1);
}
```

### 2.2 Integration-тесты

- **Цель**: SQLx-репозитории, миграции, транзакции.
- **База**: PostgreSQL в testcontainers.
- **Покрытие**: все `INSERT/UPDATE/DELETE/SELECT` в репозиториях.
- **Изоляция**: каждый test — новая транзакция + rollback.

### 2.3 E2E API-тесты

- **Цель**: полный HTTP-поток.
- **Инструмент**: `reqwest` или `httptest`.
- **Сценарии**:
  - register → login → create project → create issue → move issue → comment.
  - permissions: viewer не может обновить чужую задачу.
  - workflow: запрещённый переход возвращает 422.

### 2.4 Контрактные тесты

- OpenAPI-схема проверяется на соответствие реальным ответам.
- `schemathesis` или собственный тест на utoipa + sample responses.

## 3. Frontend тесты

### 3.1 Unit-тесты

- **Фреймворк**: Vitest.
- **Библиотеки**: `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom`.
- **Цель**: компоненты форм, hooks, utils.
- **Mock**: MSW для API, `zustand` stores.

### 3.2 Integration-тесты

- **Цель**: взаимодействие компонентов + TanStack Query + API mock.
- **Сценарии**:
  - Создание задачи через форму обновляет список.
  - DnD на доске вызывает mutation.
  - Поиск по JQL рендерит результаты.

### 3.3 E2E-тесты + скриншоты

- **Фреймворк**: Playwright.
- **Разрешения**:
  - Mobile: 375×667
  - Desktop Full HD: 1920×1080
  - Desktop 2K: 2560×1440
- **Сценарии**:
  - Регистрация и вход.
  - Создание проекта.
  - Создание задачи.
  - Kanban board: drag-and-drop.
  - Фильтры и поиск.
  - Уведомления.
  - Админ-панель.
- **Скриншоты**:
  - Full-page после каждого шага critical path.
  - Component-level скриншоты для ключевых UI-элементов.
  - Visual regression через `toHaveScreenshot()`.

### 3.4 Accessibility

- `axe-playwright`.
- Lighthouse a11y ≥ 90.

### 3.5 Performance

- Lighthouse CI.
- Web Vitals: LCP < 2.5s, INP < 200ms, CLS < 0.1.

## 4. Мануальное тестирование

- Exploratory testing перед каждым этапом.
- Проверка тёмной темы.
- Проверка локализации ru/en.
- Проверка offline-индикатора.

## 5. CI/CD

### 5.1 Pipeline

```yaml
1. lint:
   - cargo fmt --check, cargo clippy -- -D warnings
   - pnpm lint, pnpm typecheck
2. unit:
   - cargo test
   - pnpm test:unit
3. integration:
   - cargo test --features integration
   - testcontainers PostgreSQL
4. e2e:
   - docker compose up
   - pnpm test:e2e (Playwright)
5. screenshots:
   - full-page mobile/desktop/2K
   - upload artifacts
6. build:
   - docker build backend
   - docker build frontend
```

### 5.2 Артефакты

- test reports
- coverage reports
- Playwright trace + screenshots
- Lighthouse report

## 6. Структура тестов в репозитории

```
backend/
  tests/
    integration/
      user_repo_test.rs
      issue_repo_test.rs
    e2e/
      auth_flow_test.rs
      issue_flow_test.rs
  src/
    services/
      tests/
        issue_service_test.rs

frontend/
  tests/
    unit/
      LoginForm.test.tsx
      useAuth.test.ts
    integration/
      CreateIssue.integration.test.tsx
    e2e/
      auth.spec.ts
      project.spec.ts
      board.spec.ts
      screenshots/
        mobile/
        desktop/
        desktop2k/
```

## 7. Чек-листы

### 7.1 Перед PR

- [ ] Unit-тесты зелёные.
- [ ] Новая логика покрыта тестами.
- [ ] Frontend typecheck проходит.
- [ ] Линтеры чистые.

### 7.2 Перед релизом

- [ ] Integration-тесты зелёные.
- [ ] E2E critical path зелёный.
- [ ] Скриншоты по трём разрешениям.
- [ ] Visual regression diff ≤ порога.
- [ ] Lighthouse ≥ 90.
- [ ] Load tests пройдены.
- [ ] Ручная проверка темы и локализации.

## 8. Инструменты

| Задача | Инструмент |
|--------|------------|
| Backend unit | cargo test + mockall |
| Backend integration | testcontainers |
| API e2e | reqwest |
| Frontend unit | Vitest |
| Frontend integration | MSW + @testing-library |
| E2E | Playwright |
| Visual regression | Playwright screenshots |
| Coverage | cargo-tarpaulin, vitest coverage |
| Load | k6 / oha |
| Accessibility | axe-playwright |
| Performance | Lighthouse CI |
