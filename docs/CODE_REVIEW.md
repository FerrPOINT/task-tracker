# Code Review Guidelines — Task Tracker

> Стартовый документ. До конца разработки часть соглашений может измениться — актуализировать при стабилизации процесса разработки.

## 1. Scope

Правила и чек-листы для code review в Task Tracker: backend Rust, frontend React/TypeScript, CLI, docs, инфраструктура.

## 2. Цели

- Поддерживать качество кода и документации.
- Распространять знания в команде.
- Ловить баги, security-риски и архитектурные нарушения на ранней стадии.
- Блокировать попадание credentials и secrets в репозиторий.

## 3. Общие правила

- Каждый PR требует минимум одного approve.
- Критические изменения (auth, billing, DB migrations, security) — два approve.
- Автор PR сам запрашивает review и отвечает на комментарии в течение рабочего дня.
- Reviewer не мержит собственный PR.
- Конфликтные моменты обсуждаются в треде, при необходимости — созвон или ADR.

## 4. Процесс review

1. Проверить название и описание PR.
2. Проверить связь с issue/TZ.
3. Проверить diff на наличие secrets, credentials, IP, email коллег.
4. Проверить тесты и CI.
5. Прочитать код, задавать вопросы.
6. Проверить документацию, если PR меняет API/UX/архитектуру.
7. Approve или запросить изменения.
8. Автор фиксит, re-request review.
9. Merge после green CI и approve.

## 5. Backend (Rust) — чек-лист

- [ ] Код проходит `cargo check`, `cargo clippy`, `cargo fmt --check`.
- [ ] Тесты проходят: `cargo test`.
- [ ] Новые endpoint покрыты интеграционными тестами.
- [ ] Архитектурные слои соблюдены: controller → service → repository.
- [ ] DI через `AppContext` / `shaku`, нет глобальных состояний.
- [ ] Все публичные API имеют `utoipa` аннотации.
- [ ] Ошибки обрабатываются через `ApiError`, не `unwrap`/`expect` в production-коде.
- [ ] Нет SQL-инъекций: только prepared statements / ORM.
- [ ] Новые миграции имеют rollback и помечены idempotent.
- [ ] Логи не содержат sensitive-данных.

## 6. Frontend (React/TypeScript) — чек-лист

- [ ] `pnpm typecheck` чистый.
- [ ] `pnpm lint`, `pnpm prettier --check` чистые.
- [ ] Тесты проходят: `pnpm test`.
- [ ] FSD-слои соблюдены, импорты не нарушают направление.
- [ ] Новые компоненты типизированы, без `any`.
- [ ] API-запросы через `@tanstack/react-query`, ключи централизованы.
- [ ] Формы на `react-hook-form` + `zod`.
- [ ] Нет хардкода строк: i18n-ключи.
- [ ] UI доступен с клавиатуры, где применимо.
- [ ] E2E-тесты добавлены для критичных сценариев.

## 7. CLI — чек-лист

- [ ] `cargo check`, `cargo clippy`, `cargo fmt --check`.
- [ ] Команда документирована в `docs/CLI.md`.
- [ ] `--help` актуален.
- [ ] Env prefix `TASKTRACKER_` используется единообразно.
- [ ] Вывод читаемый и локализуемый, где применимо.

## 8. Документация и инфраструктура

- [ ] Изменения в API отражены в `docs/API.md`.
- [ ] Изменения в дата-модели отражены в `docs/DATA_MODEL.md`.
- [ ] Новые env vars добавлены в `.env.example` и `docs/DEPLOYMENT.md`.
- [ ] Compose-файлы синхронны с `docs/DEPLOYMENT.md`.
- [ ] ADR добавлен при архитектурном решении.

## 9. Security review

- [ ] Нет hardcoded secrets, паролей, токенов.
- [ ] Новые endpoint защищены auth/rbac.
- [ ] Входные данные валидируются на границе API.
- [ ] CORS/CSRF учтены для новых route.
- [ ] Зависимости проверены на уязвимости (`cargo audit`, `pnpm audit`).

## 10. Merge-критерии

- Green CI.
- Минимум один approve (два — для critical).
- Все review-треды resolved.
- Сообщение коммита соответствует Conventional Commits.
- Документация актуальна.

## 11. DO и DON'T

### DO

- Задавать вопросы, если код непонятен.
- Предлагать альтернативы с примерами.
- Хвалить хорошие решения.
- Указывать на нарушение существующих стандартов со ссылкой.

### DON'T

- Блокировать PR из-за субъективных предпочтений без ссылки на стандарт.
- Игнорировать security-риски.
- Approve без прочтения diff.
- Оставлять unresolved треды без комментария.

## 12. References

- `docs/CODE_STYLE.md` — стиль кода Rust / TypeScript.
- `docs/TESTING.md` — стратегия тестирования.
- `docs/ARCHITECTURE.md` — архитектурные слои.
- `docs/FRONTEND_STANDARDS.md` — frontend-соглашения.
- `docs/SECURITY.md` — security-политики.
- `docs/CI_CD.md` — pipeline и качество кода.
- `docs/AGENTS.md` — правила работы для агентов.
