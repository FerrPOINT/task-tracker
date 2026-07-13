# CI/CD — Task Tracker

## 1. Overview

Непрерывная интеграция и доставка для Task Tracker. Цель — автоматическая проверка кода, тестов, безопасности и готовности к деплою.

## 2. Platform

- **GitHub Actions** — основной CI/CD provider.
- **Self-hosted runners** опционально для E2E и нагрузочных тестов.

## 3. Pipeline

### 3.1 Trigger

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

### 3.2 Jobs

| Job | Runner | Purpose | Block merge on fail |
|-----|--------|---------|---------------------|
| `lint-rust` | ubuntu-latest | `cargo fmt --check`, `cargo clippy -- -D warnings` | ✅ |
| `lint-frontend` | ubuntu-latest | `pnpm lint`, `pnpm typecheck` | ✅ |
| `test-backend-unit` | ubuntu-latest | `cargo test --lib` | ✅ |
| `test-backend-integration` | ubuntu-latest | `cargo test --test '*'` с testcontainers | ✅ |
| `test-frontend-unit` | ubuntu-latest | `vitest run` | ✅ |
| `test-e2e` | ubuntu-latest | `playwright test` | ✅ |
| `coverage` | ubuntu-latest | `cargo tarpaulin`, `vitest --coverage` | ✅ |
| `security-audit` | ubuntu-latest | `cargo audit`, `pnpm audit` | ✅ |
| `docker-build` | ubuntu-latest | Build and push Docker images | ❌ ( informational for PRs) |
| `openapi-check` | ubuntu-latest | Проверка что `openapi.json` соответствует коду | ✅ |

### 3.3 Workflow file

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  lint-rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings

  lint-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install
      - run: pnpm lint
      - run: pnpm typecheck

  test-backend-unit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --lib --all-features

  test-backend-integration:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:17.6-alpine
        env:
          POSTGRES_USER: tasktracker
          POSTGRES_PASSWORD: tasktracker
          POSTGRES_DB: tasktracker
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      redis:
        image: redis:8.0-alpine
        ports:
          - 6379:6379
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --test '*'
        env:
          TASKTRACKER_DATABASE_URL: postgres://tasktracker:tasktracker@localhost:5432/tasktracker
          TASKTRACKER_REDIS_URL: redis://localhost:6379

  test-frontend-unit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install
      - run: vitest run

  test-e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install
      - run: pnpm exec playwright install --with-deps
      - run: docker compose -f docker-compose.yml -f docker-compose.test.yml up -d
      - run: pnpm test:e2e
      - run: docker compose -f docker-compose.yml -f docker-compose.test.yml down -v

  coverage:
    runs-on: ubuntu-latest
    needs: [test-backend-unit, test-backend-integration, test-frontend-unit]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --out Xml
      - uses: codecov/codecov-action@v4
        with:
          files: ./cobertura.xml
          fail_ci_if_error: true

  security-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-audit
      - run: cargo audit
      - uses: pnpm/action-setup@v4
        with:
          version: 10
      - run: pnpm audit --audit-level moderate

  docker-build:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v5
        with:
          context: .
          push: true
          tags: ghcr.io/ferrpoint/task-tracker:${{ github.sha }}
```

## 4. Test Isolation

### 4.1 Backend

- Каждый integration test получает **свежую логическую БД** в одном PostgreSQL контейнере.
- `testcontainers` поднимает Postgres/Redis один раз на suite.
- Миграции применяются в `setup` hook.
- Тесты внутри транзакции, откат после каждого теста (`BEGIN ... ROLLBACK`).
- Параллельные тесты отключены для integration, включены для unit.

### 4.2 Frontend

- Unit-тесты изолированы через MSW.
- LocalStorage/SessionStorage мокаются в `setupFiles`.
- Zustand store сбрасывается в `beforeEach`.
- TanStack Query cache — `queryClient.clear()`.

### 4.3 E2E

- Каждый test получает чистое состояние:
  - новый браузерный контекст
  - seed БД через API endpoint `/api/v1/test/seed`
  - deterministic fixtures
- Parallel workers = 4 (sharding по CI-нодам).

## 5. Flaky Tests Strategy

| Problem | Solution |
|---------|----------|
| Async race | `await expect(...).toPass({ timeout: 5000 })` |
| DB ordering | deterministic `ORDER BY` и сортировка по `created_at` |
| Time-based | `timekeeper` / fake timers |
| Network mocks | MSW with strict request matching |
| Unstable selectors | `data-testid`, stable text |
| Quarantine | flaky test → `@flaky` tag → отдельный CI job с retry |
| Retry policy | Playwright retry=2, unit/integration retry=0 |

## 6. Coverage Gates

| Layer | Required | CI gate |
|-------|----------|---------|
| Domain / services | >= 80% | ✅ block |
| Repository | >= 70% | ✅ block |
| API controllers | >= 75% | ✅ block |
| Frontend utils / hooks | >= 70% | ✅ block |
| Critical UI components | >= 60% | ✅ block |
| E2E critical path | 100% | ✅ block |
| Project total | >= 75% | ✅ block |

Coverage не должно падать относительно `main`.

## 7. Mutation Testing

- Backend: `cargo-mutation-testing` (или `mutagen`) для domain/services.
- Frontend: `stryker-js` для критичных pure functions.
- Запускается в отдельном scheduled job раз в неделю, не блокирует PR.
- Target: mutation score >= 60%.

## 8. Test Artifacts

| Artifact | Path | Retention |
|----------|------|-----------|
| Coverage HTML | `target/tarpaulin/` / `coverage/` | 30 days |
| Playwright report | `playwright-report/` | 14 days |
| Playwright screenshots | `test-results/` | 14 days |
| JUnit XML | `junit-*.xml` | 30 days |
| OpenAPI diff | `openapi-diff.md` | 30 days |

## 9. Pre-commit / Pre-push

### 9.1 Pre-commit (local)

```bash
# .husky/pre-commit
pnpm lint-staged
```

```json
// package.json
"lint-staged": {
  "*.{ts,tsx}": ["eslint --fix", "prettier --write"],
  "*.rs": ["rustfmt"]
}
```

### 9.2 Pre-push (optional)

```bash
# .husky/pre-push
cargo test --lib
pnpm vitest run
```

## 10. Release Pipeline

1. Создание release branch `release/v0.1.0`.
2. PR в `main` с обновлением `CHANGELOG.md`.
3. CI green.
4. Tag `v0.1.0`.
5. Docker image `ghcr.io/ferrpoint/task-tracker:v0.1.0`.
6. GitHub Release notes.

## 11. Branch Protection

- `main` требует:
  - 1 approving review
  - все CI jobs green
  - up-to-date branch
  - linear history
- Force-push запрещён.

## 12. Secrets

| Secret | Purpose |
|--------|---------|
| `GITHUB_TOKEN` | push images, create releases |
| `CODECOV_TOKEN` | upload coverage |
| `DOCKER_REGISTRY_TOKEN` | альтернативный registry |

## References

- `docs/TESTING.md` — детали стратегии тестирования.
- `docs/DEPLOYMENT.md` — Docker Compose и production деплой.
- `docs/RELEASE.md` — процесс релизов.
- `docs/SECURITY.md` — security hardening.
- `docs/AGENTS.md` — правила для разработчиков.
