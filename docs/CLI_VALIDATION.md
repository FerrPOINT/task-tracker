# Проверка CLI Task Tracker

Проверено 2026-10-01 в отдельном task checkout `feat/cli-workflows`.

## Пройденные проверки

Среда: Ubuntu WSL, Rust 1.88.0; дополнительный repository test выполнен на PostgreSQL 16 в отдельном временном кластере и БД. Sibling crates Services Base использованы без изменений в рамках этой задачи.

Из `backend`:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
TT_TEST_DATABASE_URL=postgres://... cargo test --locked -p infra --test repos deleted_issue_key_can_be_resolved_for_restore_only -- --ignored --test-threads=1
cargo run --locked --quiet -p api --bin gen-openapi
cargo build --locked --release --workspace
```

- Workspace: 508 passed, 0 failed, 21 ignored. Один из ignored repository tests дополнительно запущен на PostgreSQL и прошёл; оставшиеся ignored persistence tests целиком не запускались.
- CLI: 3 unit, 6 subprocess/HTTP workflows и 1 real API lifecycle, все прошли.
- Application test проверяет UUID/key, access denial, отсутствующие/некорректные ключи и отдельное разрешение удалённой задачи для восстановления.
- Real API: create/get UUID/get key, list/statuses/types/transitions, stdin комментария, update/unassign, worklog CRUD, link, attachment upload/list/download, transition/delete/trash/restore.
- HTTP fixtures: фильтры и страницы, разрешение ключа перед дочерним запросом, custom field JSON values, files/stdin, JSON stdout/204, error stderr/status/code/request ID, credential redaction и download no-clobber.
- Полный API suite сохраняет validation 400 даже при недоступном repository; входные поля update/transition валидируются до разрешения задачи.
- OpenAPI сгенерирован из handlers: изменены только четыре описания идентификатора, схемы DTO и operation IDs сохранены.

## Границы подтверждения

CLI real API и основные API/application tests используют production handlers/services с изолированными memory repositories и attachment storage. Отдельный SeaORM/PostgreSQL regression проверяет чтение удалённого ключа в реальном хранилище. Это не полная приёмка всех persistence сценариев PostgreSQL.

UI, Docker images и runtime окружения продуктов не изменялись. Windows native linking недоступен (`link.exe`), полные gates выполнены в WSL. Команды, примеры, конфигурация, ввод/вывод/ошибки и исключённые операции — в [CLI.md](CLI.md), изменение API — в [API.md](API.md).

Push, merge и deploy не выполнялись. Исходные незакоммиченные работы сохранены в исходных checkout.
