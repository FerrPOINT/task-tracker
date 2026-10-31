# Исторические заметки о квалификации исполнения

Сохранено из прежнего вступления ТЗ. Это датированные source-only ограничения
старых кандидатов, а не текущее состояние исходников, runtime или приёмки.
Актуальная модель и границы текущей реализации — в
[ТЗ](../TZ.md) и [плане lifecycle](../plans/unified-task-lifecycle-20261030.md).

An accepted terminal PM run before its first checkpoint may obtain a bounded
checkpoint-only permit using real saved technical Feedback. This does not answer
a business question or confirm requirements. [Source-only contract](../contracts/pm-terminal-idle-recovery.md).

A saved PM answer/feedback must survive actual lease expiry before continuation
without another card, chat or business message. Source candidate recovery uses
the current assignment and a Fleet no-successor barrier; owner confirmation and
Backlog/Ready policy remain unchanged.
[Implementation and remaining gates](../contracts/pm-settled-input-recovery.md).

Source-only расширение ручного продолжения принимает optional nine-field manual
budget только из существующего trusted Fleet witness, не из публичного POST.
Default cap3 сохраняется; дополнительные позиции ограничены
`prefix_count <= n < prefix_count + 3` с неизменными custody/fence/expired-CAS
checks и immutable history. Forward104 зарегистрирован Main без правок103.
Это не новый owner answer/confirmation, Auth role, CLI/MCP или ACK; unknown
четвёртая попытка и её latch не сбрасываются. На c8/ed61 реально проходят девять
scoped tests, включая PostgreSQL. После7b test-only conversion fix пройдены
fmt/strict Clippy и actual Rust export/client parity; standalone typecheck не
подменяет полный frontend gate. Новый image/live ещё не проверены; установленный
Tracker533 всё ещё producer blocker.
[Точный контракт](../contracts/pm-settled-input-recovery.md#optional-trusted-manual-budget-source-only).

Technical bootstrap retry before PM enrollment/first instruction is a scheduler
recovery, not a new Draft or owner answer. The existing task/chat/business input
is retained and unknown/pending effects block retry. Source candidate only:
[contract](../contracts/pm-bootstrap-technical-retry.md); no installed acceptance.
