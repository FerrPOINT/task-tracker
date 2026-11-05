# Самостоятельная QA инструментов Analyst

Проверка выполняется в отдельной disposable PDLC1 QA. Fleet и production
не участвуют. Штатные CLI работают с реальным HTTP API; тестовое назначение
подготавливается оператором, без доказательства запуска Fleet и без `Complete`.
Workflow дорабатывается отдельно: текущая квалификация не вызывает его API/CLI,
оценщика, финальный proposal или завершение текущей задачи.

## Подготовка назначения

Ignored тест `sdlc::candidates::tests::artifact_tests::prepare_standalone_analyst_tool_fixture`
использует существующую PG-фикстуру. Требуются:

- `TT_CANDIDATES_TEST_DATABASE_URL`: свежая БД с суффиксом `_candidates_test`;
- `TT_ANALYST_TOOLS_INPUT`: JSON с реальными QA `owner_subject`,
  `producer_subject`, `scheduler_subject`, `tracker_instance_id`, `routes`, `namespace`, `source`,
  `repository`, `requirements_by_case`, `scenario`, `ordinal_base`;
- `TT_ANALYST_TOOLS_OUTPUT`: новый путь для результата.

`scenario` — `complete`, `partial` или `missing`. У каждого независимого
прогона собственная БД и уникальная база ordinal (например, 1001/2001/3001),
чтобы не переиспользовать ключ синтетического исполнения фикстуры.
В standalone host entrypoint `routes` — непрозрачные синтетические значения
существующей фикстуры, без регистрации или чтения установленного Workflow.
Они не подтверждают совместимость его каталога или прохождение фаз. Subject
идентичности — UUID, без переназначения прежних локальных профилей.

Выход `history_probe` содержит отдельную задачу, непустой предыдущий результат
и назначение читателя для прямой проверки `sdlc result read`. История создаётся
существующими repository-операциями с явно синтетическими terminal receipts;
это подготовка тестовых данных, а не live-вызов Workflow/Fleet или HTTP/CLI
Complete. SQL-защиты результатов не отключаются. Исторический источник не
подмешивается к требованиям самостоятельного сценария. Оператор поддерживает
leases текущего анализа и отдельного читателя. Токен scheduler остаётся у оператора.

```sh
cargo test --locked -p infra --lib \
  sdlc::candidates::tests::artifact_tests::prepare_standalone_analyst_tool_fixture \
  -- --exact --ignored --test-threads=1
```

Один активный сценарий на БД. Подготовленный PM receipt и run binding
синтетические и явно отмечены как таковые. Никакие leases, fencing, ёмкость
или SQL-защиты тест не отключает. Каталог источников должен соответствовать
подтверждённым Admin owner-командам до первого запроса исполнения. Вложения
и длинное обсуждение можно создавать обычными API под QA-владельцем.

## Heartbeat при включённом каталоге

Оператор поддерживает назначение через штатный lifecycle heartbeat. Он
использует отдельную зарегистрированную scheduler identity с точным write
scope; аналитик этот credential не получает. Ключ heartbeat связывается с
assignment UUID и текущей lease version. Истёкшее назначение заменяется новой
фикстурой, без продления через SQL.

В проверке обнаружен отказ scheduler при дополнительном bind native task
после включения каталога пространств: scheduler ошибочно проверялся как
текущий producer. Исправление сохраняет отдельную scheduler-проверку и общий
native bind: активный профиль, права проекта, точный aggregate, подтверждённые
исходные пространства и clock остаются обязательными. Отрицательные проверки
должны включать чужую identity, чужой проект и отозванные права.

AttachmentRepo передаёт то же `GuardedDb` в чтение и публикацию артефактов,
включая PM inputs. Переход к raw connection теряет проверенный native caller
и commit clock; при включённом каталоге это приводит к отказу SQL-защиты.
Guarded-соединение сохраняется до транзакции без изменения HTTP-контракта.
Загрузка проверяет постоянную identity по текущему назначению в repository,
без дополнительного краткоживущего producer grant в service.
Это позволяет сохранять незавершённый анализ до финальной оценки Workflow;
требования к проекту, роли, run binding и живой lease сохраняются.

Миграция `m20261104_0000113_native_task_artifacts` добавляет узкий SQL port:
репозиторий после проверки назначения и содержимого связывает точную sealed
metadata с транзакцией. INSERT проверяет исходную задачу, автора и metadata;
COMMIT повторяет native clock/ACL/catalog проверки. Обычные вложения и
UPDATE/DELETE не получают машинных полномочий. Та же проверка COMMIT применяется
к разрешённому native комментарию; исторические миграции не переписываются.
Регрессия: ignored PG test
`sdlc::candidates::tests::artifact_tests::native_artifact_content_port_is_exact_and_rechecked_at_commit`.

## Связанные материалы

`sdlc discussion read --task <назначенная-task>` возвращает `related_tasks`:
прямые связи в том же проекте, без удалённых задач. Текст и обсуждение источника:
`sdlc discussion read --task <назначенная-task> --related-task <связанная-task>`.
Поле `source_task_id` сохраняет исходную задачу; `task_id` указывает прочитанный
материал. Пагинация и snapshot относятся к обсуждению выбранного материала.

Repository в одной guarded транзакции проверяет текущую assignment, project ACL,
связь и проект источника. Произвольная задача того же проекта, чужой проект,
удалённая связь/задача и отозванное членство не дают доступа. Generic human
`link list` остаётся закрытым для service account. Чтение связанного текста
не разрешает назначение, запись или скачивание его вложений: права артефактов
проверяются отдельно. Это штатный read-контракт роли, а не QA endpoint.

## Сохранение результата

Назначенная техническая роль читает `sdlc lifecycle --task <uuid>` собственным
credential для DeliveryFence. Это текущий readback назначения/lease, а не
дополнительный протокол или обязательный шаг старта PM. Поля assignment берутся
из `receipt.assignment`, expected_lease_version — из `receipt.lease.version`.

Через обычный `attachment upload` сохранить один `analysis.md`, скачать
через `attachment download` и сравнить SHA256. Проверить автора, исходную задачу,
assignment/execution/role в sealed metadata, receipt и обычный итоговый комментарий.
Повторная загрузка тех же байтов сохраняет исходную ссылку; чужая задача и
отозванный доступ не позволяют запись или скачивание. Текущая квалификация
не вызывает typed proposal или `Complete`. Фактические результаты CLI, Hermes и
содержательной проверки документа фиксируются раздельно в отчёте Base.

Ранее выполненная отдельная PG-регрессия без `Complete`:
`sdlc::candidates::tests::artifact_tests::analyst_tool_proposal_is_replayed_bound_and_comment_checked_without_complete`.
Она публикует и читает реальные sealed bytes через repository, проверяет
дедупликацию, комментарий и его повтор, отказ при неверном execution binding,
изменённом комментарии и замене hash при повторе proposal. Успешный proposal
сохраняет `Analysis / InProgress` и то же назначение. Workflow receipt в этом
тесте явно синтетический: тест не доказывает живую оценку Workflow или Fleet.
