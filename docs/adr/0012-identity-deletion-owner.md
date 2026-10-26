# ADR-0012: Durable identity-reference owner

## Status

Accepted — решение исходного компонента. Runtime/cohort/Kubernetes acceptance
и rollout подтверждаются отдельно, не статусом ADR.

## Context

Auth владеет identity и tombstone, но не знает всех продуктовых ссылок.
Удалённая native projection не доказывает отсутствие истории. Проверка только
перед транзакцией допускает writer race; повтор после потерянного ACK может
ошибочно отменить сохранённый fence. Local UUID/email не доказывают central binding.

## Decision

Использовать typed UserRepository owner port; неподдерживающий adapter возвращает unavailable. Private
check/freeze/release отделён от ordinary human/JIT: fresh central service-account
PAT, exact issuer/scope и зарегистрированный UUID coordinator. Исходное окно
полномочий не продлевается при lock/readback; deferred COMMIT guard сохраняет
тот же предел. Human admin и обычный machine scope не заменяют эту capability.

Аддитивная migration сохраняет immutable legacy issuer и durable alias/history
ledger. Unknown/malformed/unbound references приводят к unavailable, не к zero.
Все поддерживаемые writers участвуют в reference barrier; epoch исключает stale
repeatable-read snapshot. Durable operation journal связывает payload/identity/
inventory/operation; replay возвращает прежний receipt. Release сохраняет cancel
даже до freeze, поздний freeze не восстанавливает fence; автоматического TTL thaw нет.

Операторские настройки `TT_IDENTITY_LEGACY_ISSUER`, `TT_IDENTITY_DELETION_COORDINATOR_SUBJECTS`, `TT_USER_MANAGEMENT_READ_ENABLED` и `TT_USER_MANAGEMENT_MACHINE_SUBJECTS`
не выбирают issuer автоматически и не доказывают установленный cohort. Defaults,
отказы и примеры находятся в [deployment](../DEPLOYMENT.md#private-identity-deletion-owner).

## Alternatives and consequences

Удаление только native profile, временный lock без durable journal и связывание
по email отвергнуты: они теряют историю, неизвестный outcome или provenance.
Fail-closed owner может потребовать доказанного operator backfill; это ожидаемая
цена безопасного отказа. Migration/guards применяются независимо от включения
read metadata API или наличия coordinator allowlist. Полный inventory, recovery
и конкретные проверки — [IDENTITY_DELETION](../IDENTITY_DELETION.md); scoped curl
не заменяет cross-owner restart/restore/cohort и Kubernetes acceptance.
