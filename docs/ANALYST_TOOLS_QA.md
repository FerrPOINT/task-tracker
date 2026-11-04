# Самостоятельная QA инструментов Analyst

Проверка выполняется в отдельной disposable PDLC1 QA. Fleet и production
не участвуют. Штатные CLI работают с реальным HTTP API; тестовое назначение
подготавливается оператором, без доказательства запуска Fleet и без `Complete`.

## Подготовка назначения

Ignored тест `sdlc::candidates::tests::artifact_tests::prepare_standalone_analyst_tool_fixture`
использует существующую PG-фикстуру. Требуются:

- `TT_CANDIDATES_TEST_DATABASE_URL`: свежая БД с суффиксом `_candidates_test`;
- `TT_ANALYST_TOOLS_INPUT`: JSON с реальными QA `owner_subject`,
  `producer_subject`, `scheduler_subject`, `routes`, `namespace`, `source`,
  `repository`, `requirements_by_case`, `scenario`, `ordinal_base`;
- `TT_ANALYST_TOOLS_OUTPUT`: новый путь для результата.

`scenario` — `complete`, `partial` или `missing`. У каждого независимого
прогона собственная БД и уникальная база ordinal (например, 1001/2001/3001),
чтобы не переиспользовать ключ уже зарегистрированного исполнения Workflow.
`routes` берутся из реальной установленной конфигурации Workflow. Subject
идентичности — UUID, без переназначения прежних локальных профилей.

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

## Результат

Через обычный `attachment upload` сохранить один `analysis.md`, скачать
через `attachment download` и сравнить SHA256. Итоговый комментарий и typed
proposal проверяются в тестовом контексте, включая original-key readback,
повтор и неправильную привязку. Proposal не завершает стадию; `Complete`
в этой квалификации не вызывается. Фактические результаты CLI, Hermes и
содержательной проверки документа фиксируются раздельно в отчёте Base.
