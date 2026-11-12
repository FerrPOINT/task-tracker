# Единое пространство PDLC: task-tracker

Статус: **согласованная цель**, реализация в работе, runtime не активирован.
Решение владельца 2026-10-19; общий договор — services-base ADR 0010 и
`docs/contracts/UNIFIED_SPACES_V1.md` в зависимой feature-ветке.

## Ответственность

Каноническая M:N; Project+первые links одной транзакцией. Сериализация и deferred >=1 на commit, full-set CAS. Filter до pagination/count. Lifecycle barrier против archive/create/link/start и подтверждённая projection Wiki.

## Инварианты и зависимость

Admin владеет Space и policy, Tracker M:N, Auth identity/sessions/global roles.
Space 0..N Projects; Project 1..N Spaces, без основного Space. ANY allowed Space
открывает Project; A не открывает Wiki B. Read не даёт write/admin роль.
selected empty/missing policy deny; all включает будущие Space и выдаётся явно.
Archive last Space сохраняет Project readonly и links. Active B помогает только
субъекту с B и прежней write ролью; restore не снимает Project archive.
Last unlink запрещён даже конкурентно и direct DML. Данные не удаляются.

Admission сохраняет исходный deadline <=60s/token/upstream; downstream TTL не
обновляется. Expired/unavailable fail-closed, readiness отражает зависимость.
PAT/delegation/machine scopes дополнительно. Public/legacy URL не обходят правило.

## Последовательность и gates

1. Обновить owner API/data-model/architecture и версии контрактов перед кодом.
   Applied migrations/checksums и строгие V1/V2/receipts не переписывать.
2. Реализовать свою boundary и causal tests с отрицательными соседними случаями.
   Полная route/resource матрица включает raw download/Git/PAT/jobs.
3. На заполненных isolated копиях проверить additive migration/retry/rollback,
   сохранность документов/ролей/history и union подтверждённого прежнего чтения.
   Ordinary selected snapshot; all только verified platform admins. Неизвестные
   Wiki auto-admin привилегии разобрать до activation; расширение чтения показать.
4. Проверить гонки archive/create/link/start, lost ACK/reconcile/restart и
   подтверждённый drain без таймера/force stop. Wiki projection только от Tracker.
5. Scoped checks в ходе работы, один integrated gate полного compatible serving
   cohort. Backend/SDK сначала, activation/UI затем; incompatible route блокирует.
6. Dev Firebat2 installation через PDLC4 после backup/rehearsal. Rollback только
   compatible guarded cohort с новой policy/links. Served provenance и свежая IAB
   обычным/admin со screenshots после последнего изменения.

Source/tests/publication/images/migration/live — отдельные gates. Прежние 1:1
планы исторические; новая модель не меняет смысл технических receipts.
Production, PDLC1/2/3, Hermes и соседи исключены. Не смешивать с прежними Workflow
исправлениями профессий. Зависимые PR без автоматического merge.

## Общий допуск на human SDLC API (21 октября 2026)

Старый SDLC project-access читает native ACL и не заменяет общий допуск.
Human SDLC routes используют тот же verified request frame, что обычный API:
одну исходную проверку identity/role, SpaceActor, original deadline и body guard.
Прямой доступ, каталоги и счётчики фильтруются по настоящим M:N до pagination.
Чтение по пространству не добавляет native write/owner роль. Native job grants
остаются отдельным ограниченным протоколом с собственными ownership/lease guards;
машинный principal не становится human или платформенным администратором.

Утверждённое продолжение включает production после qualified dev/migration/rollback;
раннее исключение production в исходном плане относилось к предыдущему этапу.
