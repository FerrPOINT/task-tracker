# Tracker как владелец ссылок identity

Для exact source tree `f7a4dd856cfa67139f43e44998982f27fc0e1d70` пройдены
pinned Rust 1.88 fmt/clippy, library run (548 PASS, 31 ignored), 12 explicit owner
PostgreSQL tests в семи targets и named server release. Эти счётчики разных runs
не складываются; ignored behavior, включая historical ignored PG, не квалифицирован.
Установленный runtime не изменён. Реальные Auth/private RPC, cohort восьми owners,
Kubernetes и rollout остаются открытыми gates.

Private POST `/api/internal/v1/user-deletions/{operation_id}/{check,freeze,release}`
подключён вне ordinary bearer/JIT middleware. Требуется fresh verified service-account
PAT с `task-tracker:identity-lifecycle`, exact allowlisted subject из
`TT_IDENTITY_DELETION_COORDINATOR_SUBJECTS` и точный `TT_AUTH__CENTRAL_ISSUER`.
Human/PAT roles и обычные machine write scopes этого допуска не дают. Ответы
не кешируются, body limit 16 KiB, весь запрос ограничен девятью секундами.

`TT_IDENTITY_LEGACY_ISSUER` — явная операторская привязка прежних central bindings.
Startup сохраняет её неизменно до создания repository adapters; значение обязано
точно совпадать с Auth issuer. Без binding owner возвращает unavailable, не zero.
Local UUID/email не заменяют доказанное соответствие. Durable alias ledger
учитывается как история и после удаления проекции профиля.

UserRepository port для неподдерживающего adapter по умолчанию unavailable.
Настоящий PostgreSQL owner использует raw connection только через typed protocol,
берёт advisory lock714402216002 и обновляет epoch до чтения issuer/journal/count
по всем74 объявленным guarded источникам. Unknown/malformed/unbound typed refs
делают весь check unavailable. Lock/statement timeout — восемь секунд. Исходный
verified proof проверяется после lock/count I/O, до COMMIT и повторно HTTP boundary.

Freeze сохраняет точный request и стабильный fence receipt. Replay точен; другой
payload той же operation или другая active freeze конфликтует. Release durable
даже до freeze, поэтому delayed freeze не восстанавливает отменённый fence.
Автоматического thaw по времени нет.

Read-only `/api/v2/identity-users/{subject}/memberships` возвращает настоящие
проектные участия с pagination при свежем human admin допуске; это отдельная
metadata-проекция для Admin, не разрешение менять membership или читать content.

Owner PG fixtures используют только `tasktracker_identity_test` через
`TT_IDENTITY_TEST_DATABASE_URL`, собственные schemas и cleanup при ошибке.
Семь явно выполненных targets охватывают owner5, memberships1, actual typed DTO/
PM permit contracts2, private HTTP/PG boundary1 и aliases/fences/JSON по1.
Controlled Auth wire fixture в HTTP target не является genuine central Auth E2E.
Исторические ignored PG fixtures не считаются выполненными по общему library run.

Все обязательные runtime/owner/cohort доказательства остаются в
[плане](plans/2026-10-22-identity-deletion-owner.md); isolated source gates не
подтверждают production inventory или завершённый release/rollout.
