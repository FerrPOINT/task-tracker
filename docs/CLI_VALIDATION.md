# Проверка CLI Task Tracker

Проверено 2026-10-01 в отдельном task checkout `feat/cli-workflows`.

## Пройденные проверки

Среда: Ubuntu WSL, Rust 1.88.0, Node 22.23.3, pnpm 10.28.1, Python 3.12.3. Чистая опубликованная копия Services Base `main`: `c008bec701086d4f9201180ea5451f64e88ab519`. Политика зависимости от `main` сохранена. Source snapshot сверён с task checkout; исходные dirty checkout не используются для зависимости Base. Repository regression выполнен на PostgreSQL 16 и Docker `postgres:17.6-alpine` (PostgreSQL 17.6), в отдельных временных БД.

Из `backend`:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
TT_TEST_DATABASE_URL=postgres://... cargo test --locked -p infra --test repos deleted_issue_key_can_be_resolved_for_restore_only -- --ignored --test-threads=1
cargo run --locked --quiet -p api --bin gen-openapi
cargo build --locked --release --workspace
```

- Workspace: 508 passed, 0 failed, 21 ignored. Один из ignored repository tests дополнительно запущен на PostgreSQL и прошёл; оставшиеся ignored persistence tests целиком не запускались.
- После ревью и исправления redaction CLI повторно проверен: 3 unit, 8 subprocess/HTTP workflows и 1 real API lifecycle, все прошли. Дополнительная регрессия проверяет пробелы в явном token и fallback общего token при пустом явном значении; HTTP credentials и JSON-диагностика сверяются отдельно.
- Application test проверяет UUID/key, access denial, отсутствующие/некорректные ключи и отдельное разрешение удалённой задачи для восстановления.
- Real API: create/get UUID/get key, list/statuses/types/transitions, stdin комментария, update/unassign, worklog CRUD, link, attachment upload/list/download, transition/delete/trash/restore.
- HTTP fixtures: фильтры и страницы, разрешение ключа перед дочерним запросом, custom field JSON values, files/stdin, JSON stdout/204, error stderr/status/code/request ID, credential redaction и download no-clobber.
- Полный API suite сохраняет validation 400 даже при недоступном repository; входные поля update/transition валидируются до разрешения задачи.
- OpenAPI сгенерирован из handlers: изменены только четыре описания идентификатора, схемы DTO и operation IDs сохранены.

## Границы подтверждения

CLI real API и основные API/application tests используют production handlers/services с изолированными memory repositories и attachment storage. Отдельный SeaORM/PostgreSQL regression проверяет чтение удалённого ключа в реальном хранилище. Это не полная приёмка всех persistence сценариев PostgreSQL.

UI, Docker images и runtime окружения продуктов не изменялись. Windows native linking недоступен (`link.exe`), полные gates выполнены в WSL. Команды, примеры, конфигурация, ввод/вывод/ошибки и исключённые операции — в [CLI.md](CLI.md), изменение API — в [API.md](API.md).

Ветка подготовлена для отдельного PR в `main`; merge и deploy не входят в пакет. Исходные незакоммиченные работы сохранены в исходных checkout.

## Дополнительные gates перед PR

- Docs/CI contract regression: 5 tests и README structural validator, Python 3.12.3; YAML workflow разобран parser-ом.
- Frontend: `pnpm install --no-frozen-lockfile`, `pnpm openapi:check`, `pnpm openapi:compat --base-ref origin/main`, `pnpm typecheck`, `pnpm test -- --run`, `pnpm lint`, `pnpm format:check`, `pnpm build` — успешно. 45 test files / 253 tests.
- OpenAPI drift: generated handler spec совпадает с committed spec после rebase.
- CI запускает restore regression в `tasktracker_infra_test` на `postgres:17.6-alpine` с явным `TT_TEST_DATABASE_URL`. Fixture сама выбирает именно эту БД и очищает её таблицы. Остальные 20 ignored persistence tests не заявлены как проверенные.
- Timeout backend job увеличен до 45 минут: полный последовательный workspace suite с cold build почти исчерпывает прежние 30 минут. Ни одна проверка не отключена и команды gates сохранены.
- Собственные временные Docker/PostgreSQL ресурсы очищаются после проверок; постоянные Compose-стенды и runtime snapshots не используются и не изменяются.

Проверка исходной справки CLI выявила вывод значения token env variable в `--help`. В итоговой ветке `hide_env_values` скрывает значение, сохраняя имя переменной; subprocess regression выполняется с заданным fixture token и проверяет stdout/stderr. После этого изменения повторены CLI tests, Clippy и release build CLI; API/backend fixtures не меняются.
