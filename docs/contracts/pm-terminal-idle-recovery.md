# Разрешение Checkpoint После Terminal PM

Статус: реализация в исходниках; Cargo/PG/API, установка и live-приёмка открыты.

## Граница

Существующий POST execution-lease/resume принимает закрытый дополнительный вариант
с `terminal_idle_recovery`. Обычный resume сохраняет прежние wire/hash/receipts.
Смешивать варианты с обычным checkpoint или settled-input recovery нельзя.
Поддерживается только настоящий сохранённый Feedback, не confirmation.

Scheduler передаёт текущий owner/assignment/lease CAS, terminal run и hash точного
checkpoint body. Tracker сам читает witness через зарегистрированный Fleet reader:
`/internal/runtime/v1/pm/terminal-idle-recoveries/{barrier_id}`.
Утверждение вызывающей стороны не заменяет readback.
Hash запроса: canonical SHA256 от
`{"operation":"permit_pm_idle_checkpoint","payload":<request>}`.

Допуск требует single-task-v1 Draft/Brainstorming без требований и открытых
вопросов, текущего владельца/project/assignment, точного expired lease и receipt
сохранённого Feedback. Witness связывает первый Workflow step, terminal native,
identity и checkpoint body. После сетевого чтения SQL повторно проверяет условия
под существующей блокировкой задачи; транзакция не удерживается во время HTTP.

## Эффект И Повтор

Единственная SQL-запись — permit в существующий `sdlc_idempotency`. Новая миграция
не нужна. Lease остаётся expired; нет business save, outbox event, runtime dispatch
или подтверждения требований. Scope `idle_checkpoint` разрешает только точную
операцию checkpoint максимум на 30 секунд.

Повтор сохраняет permit ID и expiry. GET readback обновляет только database
`observed_at`, но не продлевает разрешение. Другой payload с тем же ключом,
пересечение с обычными resume/lease ключами и неверная custody отклоняются.
Истёкший receipt остаётся историей, а не новым разрешением.

После настоящего ACK checkpoint Fleet обязан выполнить обычный guarded resume,
lease CAS и Workflow rebind до нового runtime run. Подтверждение требований —
отдельное действие владельца. Первый этап завершается Backlog/Ready без Analyst.

## Проверки

Зарегистрированы 5 app tests и 1 ignored PG test. PG использует отдельную
`TT_PM_TERMINAL_IDLE_TEST_DATABASE_URL` с суффиксом `_pm_terminal_idle_test`.
Форматирование и diff-check пройдены; исполнение тестов, OpenAPI/client parity,
совместимая установка Fleet и полезный PM-диалог пока не подтверждены.
Fixtures не заменяют live evidence.

Согласованный wire-контракт:
[Fleet PM Terminal Idle Recovery V1](https://github.com/FerrPOINT/fleet-control/blob/docs/pm-draft-backlog-diagnosis-20261021/docs/contracts/pm-terminal-idle-recovery.md).
