# Выбор проверок и быстрый цикл Tracker

Дата: 2026-10-21. Этот checkout использует прежние исходники Tracker и
`@sdlc/ui`; настройки новых namespace/product closures из PDLC1 сюда не переносятся.
Общий стандарт — `docs/BUILD_PERFORMANCE.md` из exact quality tooling,
указанного в `../source-workspace.json` относительно корня продукта.

## Изменение

- `.quality-gate.json` задаёт docs/frontend/backend/tooling и ручной полный gate.
- Обычные CI jobs используют exact Base selector с проверкой его пакета.
  Docs-only не запускает приложения; frontend/backend выбираются независимо.
- Автоматические backend units — `cargo test --locked -p domain --lib`.
  Проверка поставляемого server и OpenAPI выполняется при production inputs.
  Остальные backend tests сохраняются в ручной полной проверке.
- `pnpm test:unit` исключает integration/slow/e2e filenames; `test:all` сохраняет
  полный прежний Vitest inventory. Base Build вызывает unit и Vite по одному разу.
- Исходный CI целиком сохранён в `release.yml`: PostgreSQL, ignored restore,
  full workspace/MSRV, OpenAPI, packed UI consumer и browser theme acceptance.
- Новый `.base-ui-revision` содержит тот же SHA, который раньше для UI задавал
  `.base-revision`. Это разделение владельцев pin без обновления версии UI/SDK.
  Lockfile, зависимости, runtime и другие products не меняются.
- Корневой `.local/` исключён из Git: собственные reports/receipts не становятся
  новым unknown source input после каждой проверки.

## Проверка и границы

`scripts/tests/test_quality_routes.py` проверяет actual manifest/selector,
невключённые optional features, unit/production selection и manual obligations.
Root команды доступны с `-Repository task-tracker -PlanOnly`; обычное выполнение
backend на Windows требует отдельно квалифицированного native adapter.
Перенос descriptor не является таким proof. Compilation/reuse/performance,
полный release, hosted CI и live acceptance проверяются отдельно.

Production source inputs остаются консервативными: новый cfg(test) layout
и product compiler-cache tuple PDLC1 не объявляются совместимыми автоматически.
