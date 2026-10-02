# Проверка CLI Task Tracker

Проверено 2026-10-02 в изолированном task checkout `feat/cli-workflows`.

## Среда и обязательные gates

Ubuntu WSL, rustc 1.88.0 (6b00bc388 2025-06-23), Node 22.20.0 / pnpm 10.28.1, Python 3.12.3. Проверено после объединения с актуальным продуктовым `main`, с чистым опубликованным Services Base `c083783a37791e277db796361203884b87828a7d`, закреплённым в `.base-revision`. Принятая в `main` политика pinned Base, Cargo `--locked` и pnpm `--frozen-lockfile` сохранена. Base и исходные dirty checkout этой задачей не изменены.

Backend: fmt, workspace/all-target Clippy с `-D warnings`, workspace tests, отдельный MSRV check (`cargo check --locked --workspace --all-targets`), OpenAPI drift и release workspace — успешно. Workspace: **511 passed, 21 ignored**, 0 failed. Workspace выполнен последовательно, как предусмотрено workflow этого продукта.

```bash
cd backend
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
cargo build --locked --release --workspace
TT_TEST_DATABASE_URL=postgres://.../tasktracker_infra_test cargo test --locked -p infra --test repos deleted_issue_key_can_be_resolved_for_restore_only -- --ignored --test-threads=1
```

Docs validators и существующие CI-contract tests проходят. Frontend: install с `--frozen-lockfile`, OpenAPI check/compat с `origin/main`, tests, lint и build — успешно. Дополнительно проходят неизменность generated contracts/lockfile, packed Base consumer и effective themes (dark/gray/light) на built preview. Task Tracker дополнительно typecheck; Task Tracker/Wiki — предусмотренный format check. Frontend tests этого продукта: 257.

## Регрессии и проверенные сценарии

- Новый subprocess regression проверяет восемь операций в table/compact/JSON, HTTP 200 с пустым body и 204, а также access denial без ложного success: 72 сценария. До исправления терялось прежнее текстовое подтверждение; после него тексты восстановлены, JSON остаётся `{"status":"ok"}`.
- Real API CLI lifecycle: UUID/key, statuses/transitions, stdin, update/unassign, links, custom fields/worklogs, attachments, delete/trash/restore; production handlers/services с memory repositories.
- Дополнительно выполнен ignored PostgreSQL test `deleted_issue_key_can_be_resolved_for_restore_only` на `postgres:17.6-alpine`, БД строго `tasktracker_infra_test`. Остальные ignored persistence tests не заявляются как выполненные.

Существующие проверки file/stdin, pages, JSON/204, access/validation/conflict, transport timeout, credential redaction и download no-clobber сохраняются и проходят. Новые regressions воспроизвели замечания на исходной ветке, затем прошли после исправлений.

## Границы подтверждения

Проверки используют только fixture данные и собственные временные ресурсы. Постоянные Compose-группы, runtime images, volumes и production deployment не менялись. Windows native linking недоступен (`link.exe`); Rust gates выполнены в WSL. Новых endpoint, миграций или изменений Services Base нет. Слияние PR в main и deploy не выполняются.

Описание команд, configuration, input/output/errors и ограничения: [CLI.md](CLI.md).
