# Интеграция Одной Задачи И Семи Профессий

## Статус

Это исходниковый пакет для совместного выпуска с Fleet Control и Workflow,
не подтверждение live-готовности. Первая версия не создаёт дочерних карточек
и integration barriers: Architect готовит архитектуру и план той же задачи.
Legacy задачи, assignments, routing и история сохраняются.

Tracker владеет задачей, requirements/confirmation, очередью, lease/fence и
переходами. Fleet хранит отдельный чат каждого конкретного агента и runtime
запуски. Workflow проверяет шаги. Forge владеет build/deployment evidence.

## API

Под `/api/v1/issues/{id}/sdlc` зарегистрированы:

| Метод / путь | Назначение |
| --- | --- |
| GET `lifecycle` | Fresh `{observed_at,receipt}` с серверными часами |
| POST `lifecycle` | Scheduler-only команда; точный replay не продлевает lease |
| GET `lifecycle/operations/{key}` | Исходный сохранённый результат команды |
| GET `lifecycle/results/{result_id}` | Входной результат предшествующей профессии |
| POST `delivery-proposals` | Предложение результата от привязанного исполнителя |
| GET `delivery-proposals/operations/{key}` | Сверка исходной подачи результата |
| POST `pm-feedback` | Сохранение свободного ответа владельца PM |
| GET `pm-feedback/operations/{key}` | Сверка сохранения PM feedback |

Все пути используют существующий strict Central Auth layer и возвращают
`Cache-Control: no-store`. POST различает создание `201` и исходный replay `200`.
Требуются реальный проектный доступ, owner/machine полномочия и точные версии.
Service writer не получает права подтверждать требования за владельца.

Agent CLI подаёт proposal до завершения своего run. Proposal не завершает стадию:
Fleet после настоящего runtime terminal и проверки Workflow передаёт Complete;
Tracker проверяет соответствие и выбирает разрешённый переход.

## Включение И Миграция

`TASKTRACKER_SDLC__SINGLE_TASK_PROJECT_IDS` — явный CSV canonical project UUID,
по умолчанию пустой. Неверные/нулевые/повторные IDs отклоняются при старте.
Новые EnsureQueued/Reserve должны проверять scope после исходного replay.
Отключение сохраняет историю и не мешает завершению уже принятого исполнения.
Repository gate интегрируется с текущим lifecycle пакетом; один env parser
не считается активацией функции.

Additive migration `000096` регистрирует историю delivery/feedback и защиты
переходов. Её SQL и canonical issue projection ещё проходят интеграцию.
Rollback приложения отключает rollout, сохраняя историю; destructive down
не поддерживается. Legacy migrations не переписываются.

## Выполненные Проверки

На Tracker `b109d0c116a69603012cfb7ad37c960ceb8f975e` и Fleet
`a6aa88d0fdafa2de2d7089b8079fc0d8ec02975d` прошли локальные locked workspace
check всех targets, formatting, 10 Tracker lifecycle units, 1 rollout-parser
test, 1 API schema/auth-surface test и Rust OpenAPI generation.
Producer сериализовал синтетические fixtures настоящими DTO; Fleet принял их
для шести профессий и Developer Rework. Ещё 2 Fleet assignment validation units
прошли. Временный Compose/Cargo target очищен Base ComposeHelper v2.

Это не PostgreSQL integration, реальный workflow/run/deployment, полный test gate
или frontend acceptance. Последующие изменения квалифицируются отдельно.

Открыты: additive SQL на заполненной истории, scoped credentials, heartbeat/tool
races и expiry recovery, canonical stage/status projection, PM feedback resume,
profile-aware Workflow/routing, реальные Forge receipts и сквозной цикл.
