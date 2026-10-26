# Tracker: владелец ссылок на identity

Статус: исходный owner-компонент `f7a4dd856cfa67139f43e44998982f27fc0e1d70`
прошёл pinned Rust 1.88 source/PG/release gates; runtime не изменён. Основа —
production user management и Base `docs/contracts/IDENTITY_DELETION_V1.md`,
SDK `ed61ec63b58fe9b60d8669c85de5ac6b5bec872b`; независимый UI pin сохранён.
Этот owner PR также содержит read-only проекцию memberships для Admin.
Это не подтверждение полной квалификации Auth/owner cohort или Kubernetes.

## Реализуемая граница

Auth координирует удаление и хранит tombstone. Tracker подтверждает отсутствие
собственных ссылок только после durable freeze всех поддерживаемых writers.
Endpoint не запускает JIT, не меняет бизнес-данные и не удаляет историю.

Локальный `users.id` не равен central subject по определению. Migration 103
сохраняет доказанные `users.central_sub` → local UUID в append-only ledger:
backfill текущих профилей и capture последующих insert/update. Отдельный kind=local
сохраняет доказанное местное происхождение и после удаления локального профиля.
Валидное связывание ещё существующего local-профиля переводит alias в central
атомарно; повторное использование UUID удалённого local-профиля запрещено.
Переименование сохраняет binding; переназначение UUID другой identity запрещено.
Ledger не содержит email и не выводит identity из сходства UUID. Старые профили
могут иметь случайные UUID; отдельно учитывается документированный
`UserId::for_central_subject` для ссылок до материализации профиля.

Migration 103 содержит ledger и foundation `identity_reference_fences.sql`: общий shared
statement guard, epoch row, immutable issuer и запрет новых скалярных ссылок
на frozen identity. Он защищает и прямой SQL backfill ledger. Реальный local-only
профиль не превращается в центральный subject по форме UUID; missing alias для
осиротевшей ссылки остаётся unknown.

Текущий source содержит полный owner count по 74 объявленным источникам и
private check/freeze/release. Сам foundation без этого inventory не позволяет
подтверждать нулевой результат по одним скалярным полям. Тест
`migration/tests/identity_fences.rs` проверяет только этот барьер: drain живого
writer, запрет JIT/link/backfill для frozen subject, обычную запись другой
identity, устаревший repeatable-read snapshot и fail-closed при missing epoch.
Этот foundation target выполнен в отдельном explicit PG gate; он не заменяет
реальные Auth/cohort/runtime проверки.

Включённый source batch `identity_json_references.sql` добавляет typed lifecycle
JSON extractor и барьеры на соответствующих journal/receipt/state таблицах.
Он обходит только перечисленные контейнеры и identity-поля действующих
контрактов: subject/автор/владелец/держатель/producer/verifier, точный issuer
узла и поддерживаемые local UserId. Markdown, произвольные metadata/document,
labels/options/custom fields и конфигурация не сканируются по сходству строк.
Глубина ограничена, повреждённое typed subject означает unknown. Этот batch
прошёл explicit `migration/identity_json_refs` PostgreSQL target. Это не
подтверждение runtime inventory, исторических ignored PG или полного cohort. `identity_json_refs.rs` отдельно
сверяет все реальные FK на users из PostgreSQL catalog со statement guards и
точным extractor, а также вложенные текущие SDLС contracts и excluded content.

## Инвентаризация

Проверяются UUID-ссылки: projects.owner_id; project_members.user_id;
issues.reporter_id/assignee_id; comments/worklogs/attachments.author_id;
issue_status_history.changed_by_id; watchers/votes; notifications recipient/actor;
notification_user_settings; audit_logs.actor_id; TOTP/password reset/OIDC mappings;
repository links.created_by и issue_creation_receipts.actor_id.

Проверяются центральные subjects и типизированные JSON receipts команд:
SDLC tasks, draft/idempotency, analysis, assignments, answers/confirmations,
execution leases, delivery, PM lifecycle/review/return, blockers, source/routing,
namespace bindings, project lifecycle и project-space commands/outbox,
space owner operations и original actor execution grants.

Текущий source owner содержит 74 объявленных guarded источника. Исторический
предварительный список из 76 таблиц включал JSON-поля без identity-семантики;
он не заменяет текущий extractor или фактический runtime inventory.
Окончательный перечень сверяется с реальным catalog наполненной БД и writers.
CustomFieldType имеет Text/Number/Select/MultiSelect/Date; произвольный текст,
labels, board columns и resource UUID не становятся identity-ссылками по форме.

Неатрибутируемые исторические ссылки дают unavailable/unknown. Они не считаются
нулём и не привязываются по email. Issuer для legacy sub-only bindings задаёт
оператор явно и сохраняет неизменно; до этого owner не подтверждает отсутствие
ссылок. Новые поддерживаемые writers обязаны сохранять достаточную provenance.

## Реализованный source-контракт

- Один owner-wide shared advisory guard всех reference writes, exclusive freeze;
  epoch row исключает обход старым repeatable-read snapshot после ожидания.
- Durable operation ledger с полным binding request/issuer/subject/inventory;
  неизменный replay receipt, cancellation до задержавшегося freeze, без TTL thaw.
- Полный count под тем же guard, включая историю и незавершённые команды;
  подтверждение inventory JSON и всех фактических writers.
- Private check/freeze/release вне обычного JIT middleware: свежая проверенная
  machine identity, allowlist subject, точный `task-tracker:identity-lifecycle`,
  исходный deadline после всех ожиданий, bounded body и no-store на всех ответах.
- Общие DTO из точного SDK ed61; независимый UI pin сохраняется.

## Записанная квалификация и открытые gates

Для exact f7a4 пройдены fmt и strict all-target clippy семи packages, unfiltered
library run (548 PASS, 31 ignored), 12 explicit PostgreSQL tests в семи owner
targets и named `server` release. Результаты разных runs не складываются;
ignored behavior, включая исторические ignored PG fixtures, не квалифицирован.
Source/own resources очищены, четыре постоянные Compose-группы не изменены.
Настоящие Auth/private RPC, полный cohort восьми owners, runtime writer/restart/
restore/rollback acceptance, population EXPLAIN и Kubernetes остаются отдельными
открытыми gates. Наличие локальных source/PG доказательств не означает rollout.

## Обязательные доказательства

Migration upgrade на наполненной БД; сохранение binding после rename/delete;
запрет rebind/UUID reuse и rollback profile+alias; все reference paths до/после
freeze; writer drain; repeatable-read serialization; missing epoch; неверный
issuer/operation/inventory; cancel-before-freeze; потерянный ACK и restart;
human/PAT/ordinary machine denial; отсутствие JIT; реальные owner/Auth HTTP;
populated query plans и bounded execution; совместимый rollback без down migration.

Тест `migration/tests/identity_aliases.rs` проверяет только provenance migration.
Он не заменяет оставшиеся owner/API/cohort gates или локальный выпуск.
