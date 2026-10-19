# Подключение общего каркаса Base

Статус: реализовано и проверено в source; установка runtime отдельна.

Канонический контракт — [план Base](https://github.com/FerrPOINT/services-base/blob/main/docs/plans/shared-application-shell-20261018.md).

Проектные группы, namespace guards, создание задач и уведомления остаются в Tracker. Сохранённый ключ tt-sidebar-collapsed передаётся общему компоненту.

Проверки: frontend unit/type/build, настоящий packed UI, права/прямые ссылки,
compact и mobile navigation. Точный UI pin находится в .base-ui-revision.
Установка runtime, backend и SDK/skills pins не входят в этот source-этап.

Точный UI-кандидат: `4b80fbfbfae48d95219011a4612dd9427a1a4b92`. Связанные проверки и ограничения
фиксируются в [отчёте Base](https://github.com/FerrPOINT/services-base/blob/main/docs/reports/shared-application-shell-20261018.md).
