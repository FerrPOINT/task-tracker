# CI / CD — Task Tracker

## 1. Выполнение проверок

GitHub Actions workflows удалены из репозитория. Автоматических запусков на
push/PR и hosted CI badge нет. Проверки выполняются локально или в согласованном
Forge pipeline; отсутствие GitHub jobs не является результатом тестирования.

Backend fmt/clippy/tests, OpenAPI drift, frontend typecheck/lint/tests/build,
coverage, аудит зависимостей, PostgreSQL/migration и browser gates сохраняются.
Область и фактический результат каждого прогона фиксируются для точного commit.
Канонические команды и требования — в [TESTING](TESTING.md).

## 2. Локальный эквивалент перед пушем

```bash
cd backend
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
cd ../frontend
pnpm typecheck && pnpm lint && pnpm test -- --run && pnpm build
# E2E против живого стека
pnpm exec playwright test --project=chromium
```

## 3. Отдельные gates

Native tests не заменяют PostgreSQL и migration checks. Игнорируемые сценарии
SDLC/start/restore/populated upgrade выполняются явно на собственных тестовых БД
по [TESTING](TESTING.md), с проверкой фактического числа выполненных cases.
Browser smoke не заменяет остальные Playwright suites и live-приёмку.
Удаление GitHub orchestration не меняет security/data и release gates.

## References

- [TESTING](TESTING.md)
- [LOCAL_SETUP](LOCAL_SETUP.md)
- [OPS_RUNBOOK](OPS_RUNBOOK.md)
