# Tracker как владелец ссылок identity

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

Проекция по умолчанию выключена: `TT_USER_MANAGEMENT_READ_ENABLED=false` или
отсутствующая переменная дают unavailable (503), а не пустой успешный ответ.
Включение задаётся только точным значением `true`; другое заданное значение
считается ошибкой конфигурации. Параметры страницы: `limit=25`, `offset=0` по
умолчанию, `limit` от 1 до 100. Ответ содержит `items/total/limit/offset`, реальные
project key/name/role, nullable `joined_at` и `SpaceRef[]`.

Нужен fresh human admin token с session ID и GET-допуском к `task-tracker`;
PAT, machine и истёкший исходный proof не дают этого допуска. CSV-переменные
`TT_NAMESPACE__MACHINE_SUBJECTS`, `TT_SPACES__ADMISSION_READER_SUBJECTS` и
`TT_USER_MANAGEMENT_MACHINE_SUBJECTS` исключают зарегистрированные machine
subjects из human-проекции, а не выдают им права. Чтение не выполняет JIT
provisioning; отсутствующий local profile даёт пустую страницу, неоднозначные
legacy bindings и недоступный источник дают unavailable. Ошибки не кешируются.

Контракт экспортируется настоящим `ApiDoc` через `api/gen-openapi` и хранится
в `openapi/openapi.json`; frontend-проекция — `frontend/src/api/generated.ts`.
Сверка generated spec и frontend-контракта выполняется существующими командами
`pnpm --dir frontend openapi:check` и `pnpm --dir frontend openapi:compat`.
Экспорт схемы и проверка её совместимости не заменяют выполнение HTTP boundary
или полноценную central Auth интеграцию.

Owner PG fixtures используют только `tasktracker_identity_test` через
`TT_IDENTITY_TEST_DATABASE_URL`, собственные schemas и cleanup при ошибке.
Временный QA-проект создаётся через Base ComposeHelper v2 с task/purpose и
собственной disposable БД; значение подключения не выводится в evidence.
Ниже — девять отдельных targets с четырнадцатью выбранными cases. Команды
выполняются из `backend` внутри этого QA-проекта; `--ignored` нужен явно:

```sh
cargo test --locked -p infra --test identity_deletion_owner -- --ignored --test-threads=1
cargo test --locked -p infra --test identity_memberships_pg -- --ignored --test-threads=1
cargo test --locked -p infra --test identity_reference_contracts -- --ignored --test-threads=1
cargo test --locked -p api --test identity_deletion_owner_http -- --ignored --test-threads=1
cargo test --locked -p migration --test identity_aliases -- --ignored --test-threads=1
cargo test --locked -p migration --test identity_fences -- --ignored --test-threads=1
cargo test --locked -p migration --test identity_json_refs -- --ignored --test-threads=1
cargo test --locked -p api --test identity_memberships_disabled_http -- --ignored --test-threads=1
cargo test --locked -p api --test identity_memberships_enabled_http -- --ignored --test-threads=1
```

Owner target содержит пять cases, typed DTO/PM permit contracts — два,
остальные targets — по одному. Отдельные HTTP-процессы memberships проверяют
конфигурацию до создания async runtime: default-off не читает и не provision-ит
профиль; enabled проверяет pagination, human/PAT/machine boundaries, запрет
неразрешённого приложения для user, canonical global-admin exemption и истечение
исходного JWT во время реального ожидания PostgreSQL с последующим fresh retry.

Реальные router/repositories/PostgreSQL работают с controlled Auth transport.
Это не genuine central Auth E2E и не приёмка cohort восьми owners. Наличие
команд и cases не означает завершённый успешный full gate. Исторические ignored
PG fixtures не считаются выполненными по общему library run. Реальные private
RPC/cohort, Kubernetes и rollout требуют самостоятельной приёмки точной версии.

Все обязательные runtime/owner/cohort доказательства остаются в
[плане](plans/2026-10-22-identity-deletion-owner.md); isolated source gates не
подтверждают production inventory или завершённый release/rollout.
