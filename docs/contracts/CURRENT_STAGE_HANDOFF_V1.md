# Явная передача текущего этапа агенту

Статус: кандидат. Подготовка и атомарное сохранение human materials реализованы
во внутреннем repository port и проверяются отдельно. HTTP-команда постановки
на исполнение, outbox/consumer и живая поставка ещё не завершены. Сохранённое
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
hash, actor, прежний/новый snapshot, ревизии, настоящий receipt, routing и source.
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
описания, вложений, ссылок или custom fields без перехода стадии.

`pending_handoff_materials` повторно проверяет нынешнего reporter/owner и доступ,
manual custody, точную итоговую ревизию, стадию/статус, материалы и текущие
routing/source policy. Старый receipt остаётся читаемой историей при replay,
но перестаёт быть текущим основанием после изменения inputs. Этот port доступен
владельцу-человеку либо существующему ограниченному read identity; он не выдаёт
assignment и не заменяет проверку приложений/данных при фактическом claim.
После межсервисного HTTP те же условия нужно проверить ещё раз в commit.
