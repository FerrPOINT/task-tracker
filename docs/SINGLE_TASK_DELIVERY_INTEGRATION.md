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
| GET `lifecycle` | Fresh `{observed_at,runtime_state,receipt}` с серверными часами |
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

SDLC работает во всех проектах. Новые черновики и запросы кандидатов
используют проверенную политику проекта. Принятые запуски сверяются
по сохранённому назначению и исходной политике.

Additive migration `000096` регистрирует историю delivery/feedback и защиты
переходов. Её SQL и canonical issue projection ещё проходят интеграцию.
Исторические миграции сохраняются.

## Выполненные Проверки

### Актуальный Локальный Gate: 15 Октября

На `2e9943f246ae500f72eb88520d7eea74a011a079`, SDK7c7b24cd, проходят
fmt, workspace/all-target check и strict Clippy, actual Rust OpenAPI parity и
полный workspace:743 tests,52 явно ignored,35 passing summaries. Все три прежних
schema/auth failures закрыты конкретными regressions, без ослабления strict wire
или machine-vs-human permissions. Временные Compose/PG/Cargo inputs очищены.

Семь ранее пройденных PG aggregates, включая populated history и Rework, имеют
отдельный unchanged-production scope на9c833eb; это не повторное выполнение
ignored cases. Frontend489 и9 current Draft browser fixtures сохраняют свои
source-bound результаты. [Общий точный receipt](https://github.com/FerrPOINT/fleet-control/blob/feat/single-task-sdlc-20261013/docs/evidence/FLEET_FOLLOWUP_49_20261015.md).

Это не установка, live owner confirmation или полный семиролевой SDLC.

### Исторический Scope

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

## Reviewer: завершение выполненного ревью

Reviewer читает назначенную Task, обсуждение и материалы с существующими правами.
После принятого финального отчёта Workflow он публикует обычный комментарий Task,
проверяет сохранённую операцию и подаёт прежний typed result proposal. Fleet
подтверждает настоящий native terminal и выполняет существующий Complete.
Оба вердикта `approved` и `rework` сохраняются как Review / Completed / Terminal
с `review_completed`; обнаруженные дефекты не являются сбоем выполненного ревью.
Назначение, lease и binding освобождаются. Следующая роль, переход в Testing/Rework
и автоматический повтор этой доработкой не создаются.

DeliveryFence собирается из свежего lifecycle непосредственно перед Start;
heartbeat может обновить `lease.version`, сохранив назначение. Неизвестную запись
сверяют по исходному ключу. `Completed` не разрешает повторно начинать стадию.
После освобождения назначения QA читает сохранённые данные как оператор, а не
повторяет assignment-bound команды роли. Workflow terminal receipt и proposal
проверяются существующими read-only операциями владельцев.

Все фазы Reviewer относятся к одному назначению и native run. Принятый
промежуточный отчёт с `complete=false` требует продолжения следующей фазы;
успешный Start не завершает работу. Машинные UUID берутся из структурированных
ответов и передаются в CLI без ручного перепечатывания.

SDK backend/CLI закреплён на Base `5f5bcb5e41d7289ded05bca72b212d58e49c445d`;
namespace/UI pins сохранены. Operation keys со слешем проверяются как целый ключ,
защита от обхода пути сохраняется. Совместимая поставка включает Forge backend/UI/CLI
с обязательной основной Task, Workflow, Fleet и полный пакет skills Base.
Паспорт поставки фиксирует точные исходники, образы и доказательства;
реализация, проверка, установка и полная Fleet-приёмка отмечаются отдельно.
