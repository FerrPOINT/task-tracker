# Явная передача текущего этапа агенту

Статус: кандидат. Подготовка и атомарное сохранение human materials реализованы
во внутреннем repository port и проверяются отдельно. HTTP preview/request/operation/cancel
подключены к строгой центральной авторизации и проходят изолированную HTTP/PG
проверку. Транспорт consumer и живая поставка ещё не завершены. Сохранённое
подтверждение не является разрешением нативного запуска.
Он уточняет [единую модель](../plans/unified-task-lifecycle-20261030.md).

## Источник состояния и основание исполнения

Tracker сохраняет стадию/статус в существующем `sdlc_tasks`; второго владельца
состояния нет. Человек и агент пользуются общим переходом. Handoff фиксирует
реальное подтверждение человеком текущих материалов. Он не считается PM run,
PM completion, результатом предыдущей роли или deployment receipt.

Два основания должны различаться явно: прежняя настоящая PM publication и новая
передача текущего этапа человеком. Старые strict envelopes не расширяются и не
заполняются выдуманными predecessor/completion IDs. Новый контекст исполнения
оборачивает текущее назначение и явное основание отдельной строгой схемой;
старый context/readback сохраняет собственный контракт и историю.

Проверка завершённого старого execution использует его точные immutable receipts,
а не равенство прежних требований новым human inputs. Истёкший lease не является
подтверждением завершения. Активные holds, unresolved effects и неизвестная
судьба запуска закрывают передачу. Старая assignment не возобновляется после
перехода под ручное владение.

## Команда человека

`POST /api/v1/issues/{id}/handoff` принимает только expected_revision,
expected_routing_policy_version, confirmed_issue_sha256, RequirementsDocument и
исходный idempotency_key. Stage, actor, assignment и previous_result_id клиент
не выбирает. Действуют текущие права редактирования, owner consent, приложения
Tracker/Fleet/Workflow и допуск к данным. Машинный credential не становится
человеком. Чтение готовности модели не требует доступа к Fleet.

Preview должен показывать модель, её ревизию, fingerprint материалов, текущую
версию routing и конкретные blockers. Наличие policy или число агентов не
означает готовность запуска. Проверяются свежие owner observations конфигурации,
закреплённый режим Workflow, исходники и полномочия; после HTTP-наблюдения
повторно проверяются input hash, revision, routing и исходный срок допуска.
Транзакцию БД не держать открытой на время межсервисного HTTP.

Состояние Ready допускает начало этапа; InProgress допускает передачу уже
начатого человеком этапа. Handoff не откатывает статус в Ready и не меняет стадию.
Для Completed требуется допустимый ручной переход: завершённый промежуточный
этап можно перевести дальше, Deployment/Completed — явно открыть в Rework.
Подтверждение передачи не объявляет процесс запущенным; runtime отображается
отдельно. Фактическое назначение/начало сохраняет обычные fencing и CAS.

## Атомарная запись и повтор

Под общим row lock задачи проверяются issue/project identity, текущий допуск,
core revision, input fingerprint, routing revision и подтверждённый idle state.
Записываются настоящий human receipt и requirements/consent, frozen routing и
source generation, новое владение, аудит и durable outbox. Все поля и связи
должны быть согласованы при commit; потеря ответа не запускает вторую операцию.
Тот же ключ с тем же command возвращает исходный receipt после проверки текущего
доступа; изменённый payload даёт конфликт.

Исторические routing/source receipts сохраняются. Новое основание не использует
таблицу PM-review generation с выдуманным blocker/PM completion. Если требуется
новая генерация, её FK указывает на реальный human handoff. Общий owner read
возвращает точное выбранное основание, а не первый подходящий проект/namespace.
Проект без namespace сохраняет ручную работу; недостающий execution source
показывается как явная причина недоступности запуска.

## Fleet и Workflow

Используются существующие scheduler/outbox, configured-cli-v1, execution ordinal,
assignment, runtime и Workflow ledgers. Новый consumer различает human entry basis
и legacy PM basis; готовая очередь не требует поддельного Analysis intent для
другой роли. Reserve/bind/result/stop/recovery проходят актуальные ограничения
приложений и реальный task/assignment fence. Старые потребители не получают
право обслуживать новую политику только потому, что их health зелёный.

| Текущая стадия | Реальный исполнитель | Режим Workflow |
| --- | --- | --- |
| Draft, Backlog | project_manager | draft |
| Analysis | analyst | analysis |
| Architecture | architect | decomposition |
| Development | developer | initial |
| Rework | developer | rework |
| Review | reviewer | delivery |
| Testing | tester | delivery |
| Deployment | devops | delivery |

Режим и его phases берутся из закреплённого Workflow catalog. Новая assignment
начинается в режиме текущей стадии; ранее вручную пройденные роли не отмечаются
как выполненные. PM на Backlog с human basis не является историческим завершённым
PM в Backlog: это новое реальное текущее выполнение. Проверка такой записи не
должна принять чужой или старый completion как текущий запуск.

Для первого human entry отсутствие previous_result_id правомерно только с точным
handoff basis. Последующее продвижение требует настоящих результатов агента и
не может снова сослаться на human entry ради пропуска текущего результата.
После изменения материалов или конфигурации новый запуск требует актуального
подтверждения; прежний receipt остаётся историей.

## Обязательная квалификация

- Ручной Analysis/Development/Rework передаётся без фальшивых предыдущих runs.
- Draft и Backlog запускают настоящее текущее PM выполнение с отдельным основанием.
- Ready и InProgress сохраняют стадию; Completed не перезапускается молча.
- Stale inputs/revision/routing, несовпадающий ключ, неизвестный исход и гонка
  с reserve/stop не меняют состояние частично и не создают дубль.
- Старый lease/assignment/result не возобновляет работу после manual takeover.
- HTTP/API, CLI, PAT и machine chain не обходят owner/application/data admission.
- Потерянный ACK/ответ проверяется по исходному operation key без нового запуска.
- Настоящие Workflow receipts, Rework, stop и recovery проверяются между сервисами.
- Полный совместимый cohort устанавливается до активации; rollback сохраняет
  новые данные и допускает только совместимые writers. Production отдельно.

## Реализованная граница сохранения материалов

`app::task_handoff::prepare_materials` строит настоящий RequirementsRevision и
Confirmation текущей стадии. `CurrentStageHandoffRepository::save_handoff_materials`
снова читает и блокирует issue, проект, пользователя и aggregate, проверяет
исходные CAS/fingerprint/routing и сохраняет всё одной транзакцией.

Аддитивная миграция113 хранит immutable `task_handoff_materials`: исходный command
hash, actor, ревизии, настоящий receipt, routing и source. Точные прежний/новый
snapshots живут только в transaction permit и удаляются до commit: повторные
подтверждения не размножают полную историю задачи в постоянном ledger.
Связь подтверждения с требованиями защищена FK. Точный transaction permit сверяет
текущего reporter/owner, текущие routing/source heads, namespace и неизменность
всех полей состояния кроме добавленного подтверждения. Он не позволяет выделить
assignment, PM execution или сменить стадию/статус. Исходные JSON-массивы истории
дополняются без нормализации прежних значений. Изменение core_revision и событие
обновления выполняются тем же owner trigger.

Одинаковый ключ возвращает исходный receipt после проверки нынешнего доступа;
изменённый payload даёт409. Ошибка после вставки требований откатывает и
подтверждение, и материалы, и aggregate. Разные команды с одной ревизией имеют
одного победителя. На этой границе ещё нет native dispatch: новый consumer должен
проверить оригинальную ревизию/материалы, актуальный допуск и свежие owner
observations перед настоящим назначением.

Сохранённый receipt не добавляется в старые PM-review generations и не создаёт
Analysis intent для другой роли. Исторические strict-контракты сохраняются.
Старый Draft source freezer остаётся отдельным; новая функция human entry не
подделывает `flow=single-task-v1`, PM completion или прошлые результаты.


### Повторная проверка сохранённого согласия

Fingerprint issue materials хранится отдельно от fingerprint команды, который
также содержит core revision. Это позволяет учесть собственное увеличение
ревизии при сохранении согласия и одновременно обнаружить последующее изменение
описания, комментариев, вложений, ссылок или custom fields без перехода стадии.
Учёт времени и навигационная position не являются новыми входными материалами.

`pending_handoff_materials` повторно проверяет нынешнего reporter/owner и доступ,
manual custody, точную итоговую ревизию, стадию/статус, материалы и текущие
routing/source policy. Старый receipt остаётся читаемой историей при replay,
но перестаёт быть текущим основанием после изменения inputs. Этот port доступен
владельцу-человеку либо существующему ограниченному read identity; он не выдаёт
assignment и не заменяет проверку приложений/данных при фактическом claim.
После межсервисного HTTP те же условия нужно проверить ещё раз в commit.


## Материалы текущей стадии и контекст исполнителя

Для Draft/Backlog PM принимает настоящую исходную цель и имеющиеся материалы,
чтобы уточнить требования. Не нужно выдумывать scope/acceptance/checklist ради
первого PM-запуска. Непустые переданные пункты и уникальность check IDs проверяются
так же, как прежде. Для Analysis и технических стадий сохраняется прежний полный
`validate_document`; начальное PM-согласие не заменяет последующее реальное
подтверждение изменённых или дополненных требований.

Исторический немаркированный Clarification без status сравнивается через общий
canonical reader как Draft/Ready с уточнением. Новое подтверждение относится к
Draft, но исходные stage/status и прежние receipts в истории не переписываются.

Новая команда использует verified human identity, включая пользовательский PAT с
нужными текущими приложениями/scopes. Наличие browser SID не подделывается; один
`human_session` без trusted human identity не открывает запись. Старый PM API и его
правило owner session остаются прежними. Машинный субъект не подтверждает за человека.

`CurrentStageExecution` — отдельная strict-схема для нового basis, с прежним
configured-cli-v1, execution ordinal, lease, run binding и fencing. Её pure seed
сохраняет Ready/InProgress и текущую стадию для всех девяти входов, не создаёт
assignment или предыдущего результата. Этот seed ещё не является queue/dispatch.
Исторические SdlcContext/LifecycleReceipt/DeliveryAssignment не расширяются.


## Durable запрос и отмена до назначения

Аддитивная миграция115 использует тот же immutable human ledger для признака
`execution_requested` и audit свежих owner observations. Новый broker и второй
владелец стадии не создаются. `request_current_stage_handoff` сохраняет согласие,
запрос и событие в одной owner-транзакции. Исходные Fleet/Forge observations
получаются вне неё и привязаны к точным task/owner/revision/materials, route/role/
mode и source policy. Временные метки не обновляются промежуточным слоем.
Deferred constraint повторно проверяет исходный срок фактов при commit; истечение
откатывает весь пакет, включая требования и подтверждение.

Queued request блокирует конфликтующую ручную запись, хотя assignment и native
run ещё отсутствуют. Исторический runtime/terminal predicate сохранён отдельно;
pending read может увидеть свой queued request, но не выдаёт разрешение запуска.
Фактический worker должен снова получить текущий допуск и свежие owner facts.

`cancel_current_stage_handoff` отменяет только текущий ещё не взятый запрос под тем
же row lock и с expected_revision. Сохраняется настоящий actor и immutable cancel
receipt, удаляется только queued pointer, общая core revision и событие меняются
атомарно. Stage/status, требования, routing/source history и чужие операции
сохраняются. Это не native stop: для взятого, running или unknown исполнения
по-прежнему нужен реальный runner-протокол и terminal/no-successor proof.

Повтор старого request/cancel возвращает исходную историю после проверки текущего
доступа и не влияет на successor. Изменение target/payload при прежнем ключе
отклоняется. Отмена вновь открывает разрешённую ручную работу; повторная передача
требует явной новой команды с актуальными материалами и ревизией.

Новые owner observations — конфигурация и исходники, а не runtime_ready/dispatch
permit. Configuration proof сохраняет hash точного agent/role/mode/route; чужое
или просроченное наблюдение не становится подходящим через новый wrapper.


## HTTP readback и отзыв запроса

- `GET /api/v1/issues/{id}/handoff` возвращает состояние модели, оригинальный
  fingerprint только для допустимого owner preview и конкретные blockers.
  Без настроенного исполнения ручная модель продолжает работать. Ошибку
  исходников нельзя представлять как отсутствие owner consent.
- `POST /api/v1/issues/{id}/handoff` возвращает 202 для нового durable запроса,
  200 для точного повтора; это не подтверждение запуска процесса.
- `GET /api/v1/issues/{id}/handoff/operations/{key}` читает исходный запрос,
  включая сохранённую отмену. Для восстановления после потери ответа не нужны
  новый ключ, новые материалы или доступность Fleet/Forge.
- `POST /api/v1/issues/{id}/handoff/{handoff_id}/cancel` отменяет только свой
  незахваченный запрос с expected_revision и исходным ключом. Снятие приложения
  Fleet не лишает владельца права отозвать очередь через Tracker. Работающий
  процесс останавливается исключительно настоящим runner stop-протоколом.

Запрос текущего кандидата требует актуальные приложения task-tracker,
fleet-control, project-workflow, ci-cd и их read/write scopes у человеческого
PAT: credential исполнения не расширяет ограниченный initiating token.
История и отмена очереди требуют Tracker; machine subject не заменяет человека.
При повторе POST сначала проверяется текущий доступ и исходный payload, затем
возвращается та же операция без повторного HTTP к peers. Новый запрос заново
сверяет материалы и первоначальные owner observations перед commit.


## CLI

`task-tracker handoff preview --issue KEY`, `handoff request --issue KEY
--from-file command.json` (или `-` для stdin), `handoff operation --issue KEY
--key ORIGINAL` и `handoff cancel --issue KEY --handoff-id UUID
--expected-revision REV --idempotency-key ORIGINAL_CANCEL` сохраняют exact
payload/revision/key. CLI не подбирает стадию, не создаёт новый ключ после ошибки
и не повторяет мутацию автоматически.


## Чтение очереди существующим scheduler

`GET /api/v1/sdlc/current-stage-candidates` использует прежний bounded query
project_id/after/limit (1…100) и возвращает только durable human requests.
Фильтры текущего pointer/revision/owner, отмены и native busy применяются до
LIMIT. Непоставленные saved-only consent, отменённая история и архивные проекты
не являются кандидатами. Это обнаружение очереди, не обещание native readiness;
материалы, policy и внешние владельцы проверяются снова перед назначением.

`GET /api/v1/issues/{id}/handoff/execution` возвращает отдельный strict
CurrentStageExecution для исходного human basis. Пока request не назначен,
assignment/lease/binding/result равны null. Ready/InProgress сохраняются, fake
predecessor/PM completion не появляются. Только зарегистрированный scheduler
с точным read credential и текущим project ACL получает это чтение; human
и произвольный read machine не наследуют эти права.

Для выбранной очереди machine reader подтверждает текущие приложения настоящего
owner вне SQL-транзакции. Затем владелец повторно проверяет тот же handoff и
проверяет каталог точного source namespace с оригинальным deadline.
Отказ приложения Fleet закрывает контекст. Старый PM/delivery source scope не
используется: у текущего human basis свой точный source receipt. Никакой
native_task_scope для записи это чтение не устанавливает; run claim не создаётся.
Самостоятельный архив проекта возвращает project_read_only в preview и исключает
его из discovery без удаления запроса; восстановление сохраняет исходный basis.
Резервирование, настоящий запуск, результаты и терминальная квитанция ещё требуют
consumer-интеграции: наличие этого readback их не подтверждает.


## Интерфейс владельца и незахваченный запрос

Preview содержит nullable `current_request`: исходную operation, текущую core revision
и вычисленное сервером `can_cancel`. Чтение своей очереди предшествует проверке
приложений запуска: отзыв Fleet не прячет уже сохранённый запрос и не мешает
его допустимой отмене. Read-only PAT не получает право отмены; архив проекта
и неподтверждённое native исполнение также закрывают отмену.

Карточка задачи открывает форму «Работа с агентом» на общих FormDialog/FormField
Base. Материалы пишет человек; подтверждение связано с revision, fingerprint
задачи, routing version и текущим содержимым формы. SSE изменения задачи/комментария
и восстановление соединения перечитывают preview. Учёт времени не меняет материалы.
Смена identity или контекста очищает форму и подтверждение.

POST и отмена не повторяются автоматически. При неизвестном исходе сохраняются
исходные command/key: пользователь может прочитать operation или явно повторить
тот же запрос. Транспорт валидирует revision, UTF-8 размер ключа и документ,
но не дублирует серверную таблицу переходов. Сохранённая очередь не называется
запущенным агентом. Реальное назначение и результат подтверждаются отдельно.


Новая очередь не зависит от legacy `single_task_project_ids`. Готовность проекта
определяют реальные routing/source и owner observations, а доступность исполнения —
настроенный сервис. Старые PM/delivery команды сохраняют прежние контракты и gates.
Отсутствие маршрутов не разрешает запуск; ручная модель остаётся независимой.


Human routing включён в общий inventory через отдельный immutable индекс116.
Generation отражает порядок frozen snapshots, а не факт PM review. Payload
не копируется: проекция читает настоящий `task_handoff_materials.routing`. При
отмене он сохраняется как история; следующий consent получает новую generation.
Legacy initial/review snapshots сохраняют прежние gates и не могут занять
уже использованный snapshot или generation. Native reservation этим не создаётся.


### Связь с исходниками вручную созданной задачи

Материалы содержат канонический набор repository identities: namespace, Forge
instance, repository ID и kind. Он объединяет явные task links и выбранный
project source. Название ссылки, автор связи и время её создания не являются
исходными материалами; остальные репозитории не скрываются. При явном запросе
очереди точная связь с project source сохраняется в той же транзакции от имени
человека. Её проекция не меняет sealed material hash. Конфликтующая identity
существующей связи отклоняется, её metadata не перезаписывается. Replay не
создаёт дубль; failed commit откатывает и эту связь.

Сохранение источника не создаёт Forge branch, agent run или результат. Actual
reservation должна отдельно связать genuine human source receipt с native
assignment и проверять актуальные права/конфигурацию.
