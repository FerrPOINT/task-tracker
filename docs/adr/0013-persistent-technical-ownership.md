# ADR-0013: Persistent Technical Assignment Ownership

## Status

Accepted for source. Coordinated qualification and production acceptance pending.

## Context

Назначение — устойчивое право продолжить конкретную работу. Таймер и restart
Fleet не доказывают завершение, отказ или разрешение создать другое назначение.

## Decision

Для текущего single-task-v1/configured-cli-v1 technical lifecycle сохранять ID,
holder, fencing и operation keys до принятого результата либо known terminal Stop.
Current `receipt.lease` имеет DeliveryOwnership с expires_at:null; claimed_at и
heartbeat_at — metadata. Отсутствие native journal продолжает исходное назначение.
Новые heartbeat/renew и expiry retry запрещены. Исторические ключи сохраняют replay.

Forward117 меняет только current ownership projection через прежние permits.
PM/Analysis history, timestamps, assignment hashes и accepted artifacts неизменны.
CAS, project rights, current actor и единственный capacity hold обязательны.

Нового cancel API нет. Unsubmitted assignment продолжается тем же владельцем;
отсутствие binding не доказывает отсутствие native custody при lost ACK.
Прежний known terminal Stop сохраняет настоящий receipt, actor/project/fence
проверки и reconciliation journal. Unknown требует исходного readback.
Отдельные request/auth freshness и job leases этим решением не меняются.

## Consequences

Зависшее назначение продолжают либо завершают после подтверждения исхода. Tracker,
Fleet и Forge обновляются согласованно. DeliveryOwnership отделён от строгого
исторического PM decoder. Регрессии проверяют старый TTL, populated upgrade,
restart/readback, сохранённую историю, stale/foreign fences, result и known terminal Stop.
