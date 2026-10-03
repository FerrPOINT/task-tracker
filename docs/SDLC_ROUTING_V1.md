# Маршрутизация ролей проекта SDLC V1

Статус: prerequisite реализован в исходном коде; runtime admission и установка не реализованы.
Владелец: Tracker. Содержимое ролей и зеркальные контракты Base не изменяются.

## Полномочия и API

Все маршруты используют существующий строгий middleware Central Auth. Локальный
JWT, обход project ACL через global admin, PM assignment grant и новые compound PAT
не допускаются. Изменение policy требует проверенной **пользовательской сессии**,
активной локальной identity с central subject и текущего `projects.owner_id`.
В legacy `Authz::require_owner` есть обход проверки в Central mode; новый строгий
SDLC API не вызывает и не наследует этот путь.
Явное членство в проекте разрешает чтение, но не изменение policy. Строки пользователя,
проекта и членства удерживаются под блокировками до конца транзакции; replay повторно
проверяет текущие полномочия.
Машинное чтение требует ровно существующий scope `task-tracker:read`, который
выпускает issuer, и активное явное владение проектом или членство в нём.
Права делегирования и запуска при этом не выдаются.

| Endpoint | Результат |
| --- | --- |
| `POST /api/v1/projects/{project_id}/sdlc/routing-policy` | 201: новая неизменяемая revision; 200: точный replay |
| `GET /api/v1/projects/{project_id}/sdlc/routing-policy` | Текущая сохранённая policy |
| `GET /api/v1/projects/{project_id}/sdlc/routing-policy/versions/{version}` | Точная историческая policy |
| `GET /api/v1/projects/{project_id}/sdlc/routing-policy/operations/{idempotency_key}` | Исходная revision для текущего владельца с пользовательской сессией, автора и ключа |
| `GET /api/v1/issues/{id}/sdlc/routing-snapshot` | Неизменяемый snapshot точной публикации; 404 без явного opt-in |

Успешные ответы policy/snapshot имеют `Cache-Control: no-store`. Ошибки: 401 при
непроверенных credentials, 403 при недостаточных полномочиях, 404 при отсутствии
сохранённого ресурса, 409 при stale CAS, конфликте ключа или несогласованной истории,
422 при неверных refs, 503 при недоступном Central Auth или ненастроенном SDLC.
Неизвестные поля JSON отклоняются. Rust handlers/DTO являются источником OpenAPI;
отдельный вручную поддерживаемый протокол не вводится.

## Объявленные ссылки

Команда policy: `{expected_version: null | positive integer, routes, idempotency_key}`.
Поле `expected_version` обязательно: `null` создаёт version 1 только при отсутствии
policy. Обновление требует точной текущей версии; backend увеличивает её, вычисляет
канонические routing/command hashes и сохраняет один receipt на project/central-author/key.
Два конкурентных обновления не могут продвинуть одну версию. Исторический replay
возвращает исходную revision и не откатывает head. Границы версии: `1..9007199254740991`.

`routes` является строгим объектом ровно с семью полями, каждое содержит `RoleRoute`:

| Роль | Символ namespace | Ключ Workflow | Объявленный profile |
| --- | --- | --- | --- |
| `project_manager` | `hermes-project-manager` | `hermes-sdlc:project_manager` | `hermes-sdlc-project-manager` |
| `analyst` | `hermes-analyst` | `hermes-sdlc:analyst` | `hermes-sdlc-analyst` |
| `architect` | `hermes-architect` | `hermes-sdlc:architect` | `hermes-sdlc-architect` |
| `developer` | `hermes-developer` | `hermes-sdlc:developer` | `hermes-sdlc-developer` |
| `reviewer` | `hermes-reviewer` | `hermes-sdlc:reviewer` | `hermes-sdlc-reviewer` |
| `tester` | `hermes-tester` | `hermes-sdlc:tester` | `hermes-sdlc-quality` |
| `devops` | `hermes-devops` | `hermes-sdlc:devops` | `hermes-sdlc-operations` |

Каждая ссылка содержит `agent_id` (конкретный ненулевой UUID), `fleet_config_revision`
(положительное безопасное целое), `package_commit`, `package_manifest_sha256`,
`namespace_id`, `namespace_name`, `workflow_id`, `workflow_key`, `profile`,
`workflow_catalog_version`, `workflow_catalog_sha256`. Все семь agent IDs,
namespace IDs и workflow IDs должны различаться. Числовые IDs передаются каноническими
положительными десятичными ASCII-строками в пределах signed bigint, а не символами Base.
Версия каталога: 3. Точный package commit: `4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58`;
все роли объявляют одинаковые manifest/catalog SHA-256 в нижнем регистре.

Это выбранные владельцем **ссылки**, а не аутентифицированные наблюдения Fleet/Workflow.
Структурная проверка и public pin не доказывают существование или установку объектов.
Сохранённая policy всегда имеет `verification: declared`, `native_ready: false`,
`dispatch_allowed: false`. Observation UUID и native attestation не выдумываются.
Для будущего admission необходимо получить свежие наблюдения соответствующих owners,
сверить все конкретные identity/config/package/Workflow refs и отдельно проверить
native capabilities. Запись policy не устанавливает конфигурацию, не выделяет
assignment, не захватывает lease и не запускает dispatch.

### Соответствие counterpart DTO

Для будущего consumer источником сопоставления служит Fleet
`GET /internal/runtime/v1/agents/{agent_id}/configuration`. Его `workflow_binding`
содержит frozen mapping, сверяемый Fleet со свежим Workflow owner readback
`GET /internal/runtime/base/namespace-bindings/{namespace_id}`; отдельный ответ
Workflow имеет envelope `{ok: true, binding: ...}`. Соответствие полей `RoleRoute`:

| Tracker | Fleet configuration observation |
| --- | --- |
| `agent_id` | `agent_id` |
| `fleet_config_revision` | `effective_revision`, никогда не desired revision или номер подготовленного draft |
| `package_commit` | `package.commit`; также должен совпасть с `workflow_binding.skills_revision` |
| `package_manifest_sha256` | `package.manifestSha256` |
| `namespace_id`, `namespace_name` | `workflow_binding.namespace_id`, `workflow_binding.namespace_name` |
| `workflow_id`, `workflow_key` | `workflow_binding.workflow_id`, `workflow_binding.workflow_key` |
| `profile` | `workflow_binding.profile`; также должен совпасть с `package.profile` |
| `workflow_catalog_version` | `workflow_binding.catalog_version` |
| `workflow_catalog_sha256` | `workflow_binding.catalog_sha256`, не hash полного source-export envelope или его `sourceSha256` |

Fleet wire `sdlc_role` сохраняет legacy значение `dev_ops`. Только на границе
counterpart DTO оно нормализуется в канонический Tracker/package/Workflow role key
`devops`; остальные role keys не меняются. Нормализованная роль должна совпасть
с ключом в `routes`, `package.role` и `workflow_binding.role_key`. Эта нормализация
не переименовывает Fleet enum, исторические данные или сохранённые legacy bindings.
Числовые namespace/workflow IDs берутся из binding, а не выводятся из символических
имён. Hashes относятся к соответствующим owner artifacts, не к JSON всего наблюдения.

**Tracker consumer этих observations и fresh admission ещё не реализованы.**
Таблица фиксирует контракт будущего сопоставления, а не уже выполняемый remote read
или проверку при записи policy. Текущие ссылки остаются `declared`; `runtime_ready:false`
в counterpart, совпадение refs и file/config proof не разрешают native execution.
Следующие обязательные owner interfaces описаны в [lifecycle](SDLC_LIFECYCLE_V1.md).

## Snapshot точной публикации

Существующее точное подтверждение владельца принимает необязательное поле
`expected_routing_policy_version`. Положительное значение явно включает routing
для этой публикации. Под блокировками issue/task и project authorization оно должно
совпасть с текущим head policy. Транзакция сохраняет всю policy как неизменяемый
snapshot, связанный с task, root/project/instance, точными requirement revision/hash
и confirmation UUID; Analysis intent ссылается на него через реляционный FK.
Consent, snapshot, intent, aggregate/status history, receipt и два существующих
outbox-события фиксируются вместе. Любая ошибка, включая stale policy или отказ
записи outbox, откатывает все эти изменения.

Отсутствие поля или `null` сохраняет исторический hash команды confirmation и прежний
результат. Создание или изменение project policy не включает routing для существующих
задач, не переписывает PM data, не дополняет старые confirmations и не изменяет task
snapshot. Уже поставленные в очередь задачи без snapshot остаются legacy и не подходят
для будущего routed admission. Добавить snapshot к уже опубликованной задаче нельзя.
Replay старого consent не обращается к новому head policy и не заменяет frozen policy.
Изменённая opt-in версия с прежним consent key даёт конфликт. Новая публикация может
явно выбрать новую текущую версию; редактирование policy не переназначает текущую задачу.

**Текущий UI confirmation не передаёт `expected_routing_policy_version` и не включает
routing автоматически.** Этот срез предоставляет явный opt-in через API. UI настройки
project policy и выбора версии при confirmation остаётся следующим интерфейсом;
текущий UI не изменяется.

Readback проверяет сохранённые policy/hash, task/root/instance и точный confirmation.
Это только configuration/routing prerequisite. Analysis/Ready по-прежнему означает
очередь, ожидающую admission. [Prepared reservation](SDLC_RESERVATION_V1.md) теперь
фиксирует assignment/capacity/lease только в Tracker. Свежий native admission,
runtime ACK, DAG/coverage, barrier, terminal transitions и dispatch не реализованы.

## Хранение и проверки

Расширена только pending migration 000034. Policy revisions, operations и task snapshots
неизменяемы; изменяемый project head продвигается на единицу. Primary/unique keys
индексируют project/version, project/author/key и поиск task/snapshot. Составные FK
запрещают cross-project snapshot и ссылку на другой confirmation. SQL проверяет семь
различных role bindings и те же публичные ограничения refs, что application.
Insert gate публикации требует текущий root до публикации и точный head.
Срез не меняет shared/accepted schema и не мигрирует исторические данные.

Scoped тесты: `app::sdlc_routing::tests`, `api::routes::sdlc_routing::tests` и существующий
изолированный HTTP/PostgreSQL тест
`postgres_http_clarification_ownership_replay_gate_and_restart` с
`support/routing_policy.rs`. Проверены строгие refs, совместимость legacy hash,
полномочия owner/admin/member/machine, отзыв ACL, concurrent replay/CAS, stale publication,
атомарный rollback, readback после restart, неизменность старых snapshots, применение
policy только к новым публикациям, legacy opt-out и прямые отрицательные SQL проверки.
Синтетические публичные metadata тестов не являются доказательством live configuration/admission.

Финальный scoped прогон 2026-10-03: 6 тестов PASS, штатный Rust OpenAPI exporter
и `cargo fmt --all -- --check` PASS. Использован временный Compose project
`sdlc-qa-b-sdlc01-routing-9815bd65` с owner/purpose labels; контейнеры и сеть удалены
в finally. Локальное evidence, не включаемое в Git: `.local/b-sdlc01-routing-9815bd65/scoped.log`.
После экспорта прошли `pnpm generate:api`, `pnpm openapi:check` и scoped typecheck
существующих B-SDLC-05 consumers с pinned Base `9408802`. Индекс README проверен
`scripts/verify_readme.py`; Docker grouping audit не выявил нарушений.

Полный build/CI, установка и межсервисная/native приёмка НЕ ЗАПУСКАЛИСЬ.
