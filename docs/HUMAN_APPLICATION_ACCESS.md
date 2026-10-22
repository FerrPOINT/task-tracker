# Две роли и приложения: интеграция Tracker

Статус: исходники feature-ветки; до dev-поставки и активации.
Канон: Base `docs/platform/HUMAN_APPLICATION_ACCESS.md`, ADR0011.

Central Auth владеет человеческой ролью и приложением `task-tracker`.
Локальный профиль сопоставляется по неизменному central subject; JIT не выдаёт
systemadmin и не переписывает исторические назначения. Ответ собственного
профиля показывает текущую роль, сохранённые флаги не являются authority.

Обычные задачи, комментарии, вложения и значения полей остаются рабочими
операциями. `Authz` отделяет их от настройки структуры: проекты/участники,
справочники, определения полей, labels/components/versions, постоянное удаление
и административное исправление чужих записей требуют текущего central admin.
Само наличие приложения либо собственность проекта не выдаёт эту роль.

Bearer middleware связывает проверенную центральную authority с локальным UUID
в request SDK scope. SDLC имеет собственный strict middleware: human scope
связан с центральным UUID, PM/delivery/machine principals не получают human role.
Human confirmation и работа исполнителя не превращены в admin-only операции.
Управление source/routing policy и привязками репозиториев требует admin;
технические scopes/fences и строгие версии execution contexts сохраняются.

Namespace admission независим и обязателен при интеграции; текущий feature
checkout не заменяет незавершённые ветки пространств. Рабочие роли SDLC и
профессии Workflow не являются человеческими ролями платформы.

Открытые проверки до активации: адресные SQL/API tests, транзакционные proof
expiry guards, приложения всей Tracker/Fleet/Workflow/Forge цепочки,
скачивания и фоновые стадии, клиентские controls/cache, filled migration,
совместимый cohort, dev/IAB и реальное задание. Scoped source-check не означает
выполненную поставку или проверку этих границ.

## Совмещение с опубликованным срезом пространств

Интеграция выполнена только в этой task-ветке; dirty checkout и ветки владельца
пространств не изменяются. Совместимый Rust SDK сохраняет один исходный срок
identity/application/space proof. Текущий central admin определяется из решения
Auth, а не из сохранённого флага или прежнего назначения продукта. Локальные
membership/edit ограничения данных сохраняются для обычной работы; они не
позволяют управлять общей структурой без роли администратора.

HTTP-ответы и потоки ограничены исходным сроком, включая idle body. Транзакционные
и межсервисные эффекты требуют самостоятельной интегрированной квалификации;
проверка типов или доступность сервиса её не заменяют. Новая app policy остаётся
выключенной до полного совместимого cohort и dev-приёмки. Production-поставка
другой задачи не входит в разрешённый объём этой реализации.

## Подключение общего UI допуска

RequireAuth подключает общий допуск task-tracker и тематический отказ 403.
Проектные настройки и административная страница требуют текущего admin.
Создание/удаление проекта и структурные действия скрыты у user; навигация,
рабочие задачи и чтение корзины остаются доступны по прежнему допуску к данным.
OpenAPI middleware привязывает headers и поток body к поколению общего допуска;
поздний ответ не восстанавливает данные после отзыва.

Это source-реализация; полный административный matrix, серверные эффекты,
миграционная квалификация и dev/IAB остаются отдельными этапами приёмки.

## Исходный срок на COMMIT

GuardedDb привязывает исходный valid_until к каждой защищённой write-транзакции,
включая human frame без space admission. Создание проекта через typed permit
привязывает тот же срок отдельно. Новая аддитивная миграция сохраняет deferred
событие вставки fence; строка удаляется внутри транзакции и не накапливается.
Проверка clock_timestamp выполняется в БД при COMMIT, после ожиданий. SQLSTATE
42501 остаётся Forbidden. Проверка в Rust до commit является дополнительной.

Native SQL target использует исходный production adapter/error mapper и отдельную
синтетическую БД. Он проверяет время внутри COMMIT, rollback эффекта и следующий
свежий commit; это не полная ingress/Git/PAT/job приёмка. Старые serving images
без fence не являются совместимым rollback после активации. Прямые typed owner
ports и внешние эффекты требуют собственной проверки текущего proof.

## Tracker и Fleet в одной операции

Назначение Fleet и изменения execution lease требуют одновременно task-tracker
и fleet-control у текущего человека. Strict SDLC ingress использует общий
VerifiedCentralToken.require_applications: исходный Auth proof, срок и scopes
PAT не заменяются локальным флагом или служебным credential. Технические
principals остаются в отдельных PM/delivery протоколах. Source adapter в
NativeRequest получает immutable owner_subject из уже подтверждённого task
aggregate; scopes, текущие assignment/lease и machine identity сохраняются.
Для native PM/delivery операции требуются task-tracker и fleet-control у
владельца. Приложения остальных фактических этапов проверяются их владельцами;
это не заменяет полный Tracker/Fleet/Workflow/Forge matrix.

При обязательной app policy используется отдельный Auth reader credential
TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN с application-access:read.
Секрет не хранится в задаче или profile; единственный configured HTTP client
получает текущий owner proof один раз на request. Следующий stage/request
проверяет владельца заново. Ошибка Auth закрывает операцию; истёкший proof не
обновляется внутри принятой команды. Policy OFF остаётся явной совместимостью
и не создаёт запросов к owner reader.

Owner clock сужает имеющийся native window, общий transaction commit fence и
response/attachment stream. Он не создаёт human context, role или session для
служебного субъекта. Private clock и исходные scopes не меняют самостоятельный
namespace/data admission. Native ingress, actual owner readback и COMMIT/stream
квалификация этой новой композиции ещё должны быть исполнены до активации.

Обычные Tracker history, комментарии и подтверждение требований не требуют
Fleet сами по себе. Подтверждение не считается автоматическим запуском Analysis.
CreateTaskAction открывает новый PM-чат и потому требует Fleet; при отсутствии
приложения показывает причину, а отзыв во время чтения проекта отменяет переход.
Прямая рабочая форма Tracker /issues/create сохраняет собственный task-tracker
и data admission. Полная цепочка реального задания остаётся отдельной приёмкой.
