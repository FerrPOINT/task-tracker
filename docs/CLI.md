# CLI — Task Tracker

Поставка sdlc1 принята 2026-10-03: три backend обновлены после fresh backup, QA/rollback, live acceptance и наблюдения. [Установка](CLI_INSTALL.md), [проверки и ограничения](CLI_VALIDATION.md). Команды/API/exit codes сохранены; Linux x86_64/WSL, без version bumps/tags/public release.

Консольный клиент для работы с API. Бинарник: `task-tracker`.

## Установка

```bash
cd backend
cargo install --path cli
```

## Глобальные флаги

```
--api-url   Базовый URL API (env: TASKTRACKER_API_URL, default: http://localhost:7721/api/v1)
--token     Личный токен Central Auth (env: TASKTRACKER_TOKEN или SDLC_API_TOKEN)
```

## Аутентификация

```bash
task-tracker auth whoami
```

## Проекты

```bash
task-tracker project list
task-tracker project create --key PROJ --name "Project Name"
task-tracker project get PROJ
task-tracker project update PROJ --name "New Name"
task-tracker project delete PROJ
```

## Задачи

```bash
task-tracker issue create --project-key PROJ --summary "Fix bug" --issue-type task
task-tracker issue get PROJ-1
task-tracker issue update PROJ-1 --summary "Updated" --status-id <uuid>
task-tracker issue delete PROJ-1
task-tracker issue transition PROJ-1 --to <status-uuid>
```

## Примечания

- Личный токен создаётся в Admin Panel и передаётся через `--token`, `TASKTRACKER_TOKEN` или `SDLC_API_TOKEN`. Локального парольного входа CLI больше нет; обычный выход из браузера не отзывает личный токен.
- HTTP-транспорт предоставляется `pdlc-cli-core` из соседнего `services-base`; удалённые URL требуют HTTPS.
- Парсинг ключей (`project get PROJ`, `issue get PROJ-1`) происходит на стороне сервера.
- Ранее существовавшие группы команд сохранены; рабочий пакет дополняет их командами ниже.

## Ссылки

- `docs/API.md`
- `docs/ARCHITECTURE.md`

## References

- [ARCHITECTURE](ARCHITECTURE.md)
- [LOCAL_SETUP](LOCAL_SETUP.md)


## Ввод, вывод и ошибки рабочих команд

`--output json` пишет только JSON в stdout. Успешный пустой ответ нормализуется в `{"status":"ok"}`. `--error-format text|json` выбирает stderr, по умолчанию `text`. JSON-ошибка имеет вид:

```json
{"error":{"status":403,"code":"FORBIDDEN","message":"Недостаточно прав","request_id":"req-123"}}
```

При transport error `status` и недоступные API-поля равны `null`. Токен и переданные secret values исключаются из диагностики; произвольное тело ошибочного ответа не печатается. HTTP-статус хранится в JSON, числовые exit codes сохранены. Ошибки парсинга CLI в JSON-режиме имеют code `CLI_USAGE` и не повторяют входные значения; `--help` сохраняет обычный вывод.

`--from-file PATH` читает UTF-8; `--from-file -` читает stdin. Inline-текст и файл взаимоисключающие. Ввод сохраняет переводы строк. Download сначала получает успешный ответ, записывает временный файл в каталоге назначения и переносит его в итоговый путь. Существующий файл сохраняется без `--overwrite`; автоматического создания каталогов нет.

Пагинация явная: команда получает одну страницу. Повторов write-запросов и автоматической загрузки всех страниц нет. Настройки transport реализованы в продуктовом CLI; Services Base не изменён.


## Повседневные сценарии

Существующие аргументы задач принимают UUID или ключ `PROJ-1`. Сервер разрешает идентификатор через application service с проверкой доступа; дочерние CLI-команды получают UUID перед обращением к соответствующему API. Восстановление ключа использует отдельное чтение удалённой задачи с проверкой доступа к проекту.

```bash
task-tracker issue create --project-key PROJ --summary "Задача" --from-file description.md
task-tracker issue update PROJ-1 --from-file - --unassign
task-tracker comment add --issue-id PROJ-1 --from-file comment.md
task-tracker statuses
task-tracker issue-types
task-tracker transitions --issue PROJ-1
task-tracker issue list --project-key PROJ --q fix --priority high --limit 25 --offset 0
task-tracker link add --issue PROJ-1 --target-key PROJ-2 --link-type relates
task-tracker link list --issue PROJ-1
task-tracker link delete <link-id>
task-tracker attachment list --issue PROJ-1
task-tracker attachment upload --issue PROJ-1 --file evidence.txt --content-type text/plain
task-tracker attachment download <attachment-id> --out evidence.txt
task-tracker attachment delete <attachment-id>
task-tracker field list --project-key PROJ
task-tracker field values --issue PROJ-1
task-tracker field set --issue PROJ-1 --field <field-id> --value '"Значение"'
task-tracker field set --issue PROJ-1 --field <field-id> --from-file value.json
task-tracker worklog list --issue PROJ-1 --limit 25 --offset 0
task-tracker worklog create --issue PROJ-1 --started-at 2026-10-01T10:00:00Z --duration-seconds 1800 --from-file work.md
task-tracker worklog update <worklog-id> --duration-seconds 2400
task-tracker worklog delete <worklog-id>
task-tracker sdlc context --task <issue-uuid>
task-tracker sdlc input --task <issue-uuid>
task-tracker sdlc questions --task <issue-uuid>
task-tracker sdlc requirements --task <issue-uuid>
task-tracker sdlc question publish --task <issue-uuid> --from-file question.json
task-tracker sdlc revision publish --task <issue-uuid> --from-file -
task-tracker issue delete PROJ-1
task-tracker trash --project-key PROJ --limit 25 --offset 0
task-tracker issue restore PROJ-1
```

Issue filters: `--q`, `--priority`, `--status`, `--assignee-id`, `--sort-by`, `--sort-order`, `--limit`, `--offset`. Сервер валидирует значения сортировки и диапазоны. `search global`, `search jql`, `board backlog`, `notification list`, `comment list` принимают `--limit/--offset`; `notification list --include-read` включает прочитанные. `transitions` показывает переходы из текущего статуса, не обещая разрешение перехода: окончательная валидация выполняется сервером. `field set` принимает JSON, включая `null`, а не неявно преобразованную строку.

MIME type upload определяется по имени файла; `--content-type` переопределяет его. Сервер сохраняет свой allowlist допустимых типов.

`--unassign` передаёт JSON `null` и конфликтует с `--assignee-id`. Для comment add/update поддержаны прежний inline body и `--from-file`. Форматы `json|table|compact` сохранены; default `json`. Ошибки выполнения возвращают прежний код `1`, ошибки CLI parsing — `2`.

### SDLC PM producer commands

`sdlc` — тонкая обвязка над существующими typed SDLC API. `--task` принимает
только canonical issue UUID и не выполняет обычный `GET /issues/{key}` lookup:
PM machine-token имеет task-scoped grant, а не общий доступ к задаче по ключу.
Команды чтения:

```bash
task-tracker sdlc context --task <issue-uuid>
task-tracker sdlc input --task <issue-uuid>
task-tracker sdlc questions --task <issue-uuid>
task-tracker sdlc requirements --task <issue-uuid>
```

Команды публикации принимают строгий JSON body существующих серверных DTO и
отправляют его без дополнительных headers, operation receipt routes, retries или
локального вычисления revision/hash:

```bash
task-tracker sdlc question publish --task <issue-uuid> --from-file question.json
task-tracker sdlc revision publish --task <issue-uuid> --from-file -
```

Для machine publication действуют серверные проверки ownership/project scope,
machine scope, assignment, lease/fence и idempotency key внутри JSON body.
Owner-only answer/confirm/evidence команды не входят в PM CLI producer packet.

### Analysis: подготовленное назначение и результат

Команды Analysis также принимают только canonical UUID задачи. Они используют
существующие Tracker endpoints и настроенные CLI credentials:

```bash
task-tracker sdlc intent --task <issue-uuid>
task-tracker sdlc analysis preflight --task <issue-uuid>
task-tracker sdlc analysis reservation read --task <issue-uuid>
task-tracker sdlc analysis reservation reserve --task <issue-uuid> --from-file reservation.json
task-tracker sdlc analysis reservation heartbeat --task <issue-uuid> --from-file -
task-tracker sdlc analysis reservation operation --task <issue-uuid> --key <idempotency-key>
task-tracker sdlc analysis result submit --task <issue-uuid> --from-file result.json
task-tracker sdlc analysis result operation --task <issue-uuid> --key <idempotency-key>
```

JSON передаётся без изменения. Scope, owner, revision, fence и lease проверяет
сервер; CLI не создаёт proof и не запускает Fleet. После неопределённого исхода
write сначала используется соответствующий `operation` readback с прежним ключом.
Автоматического повторения POST, heartbeat или dispatch нет. Положительный
preflight сам по себе не означает принятого dispatch или завершённого Analysis.

### Single-task: чтение назначения и принятого результата

```bash
task-tracker sdlc lifecycle --task <issue-uuid>
task-tracker sdlc result read --task <issue-uuid> --result <previous-result-uuid>
```

Это readonly GET существующих `/issues/{uuid}/sdlc/lifecycle` и
`/issues/{uuid}/sdlc/lifecycle/results/{result_uuid}` относительно `/api/v1`.
Первый возвращает текущий `LifecycleReadback`; UUID предыдущего результата берётся
из actual assignment, не из display key. Второй возвращает серверный `DeliveryInput`
без подмены typed output содержимым attachment. Attachment list/download сохраняют
существующие проверки accepted predecessor chain.
Обе команды используют настроенный CLI token; серверная авторизация не меняется.
В исходном Tracker402 delivery-grant policy ещё не разрешает nested result GET:
такой 403 остаётся ошибкой, не поводом использовать scheduler token или fallback.
Ошибки не вызывают retry, POST или смену identity. Existing result propose/operation
не изменены. HTTP contract tests добавлены в `backend/cli/tests/workflows.rs`;
их выполнение и приёмка с actual Central auth требуют отдельной проверки.

### Реестр сценариев и проверок

Пути ниже относительно `/api/v1`; параметры UUID разрешаются из ключа до дочернего запроса.

| Сценарий / команда | Публичный API | Параметры и проверка |
|---|---|---|
| issue create/get/update/delete/transition/restore | POST `/issues`; GET/PATCH/DELETE `/issues/{id}`; POST `/issues/{id}/transition`, `/restore` | UUID/key, текст файла/stdin, unassign; real_api lifecycle, application access test |
| issue list, search global/jql, backlog, notifications | GET `/issues?project_key={key}`, `/search`, `/search/jql`, `/projects/{key}/backlog`, `/notifications` | фильтры и limit/offset; workflows request assertions |
| statuses / issue-types / transitions | GET `/statuses`, `/issue-types`, `/transitions` | текущий статус; workflows filter test |
| link list/add/delete | GET/POST `/issues/{id}/links`; DELETE `/issue-links/{id}` | target key и link type; workflows + real_api |
| attachment list/upload/download/delete | GET/POST `/issues/{id}/attachments`; GET `/attachments/{id}/download`; DELETE `/attachments/{id}` | multipart, content type, atomic/no clobber; workflows + real_api |
| field list/values/set | GET `/projects/{key}/custom-fields`, `/issues/{id}/custom-fields`; PUT `/issues/{id}/custom-fields/{field}/value` | JSON inline/file/stdin; workflows |
| worklog list/create/update/delete | GET/POST `/issues/{id}/worklogs`; PATCH/DELETE `/worklogs/{id}` | дата, duration, описание; workflows |
| sdlc context/input/questions/requirements | GET `/issues/{uuid}/sdlc/context`, `/pm-draft-input`, `/clarifications`, `/requirements` | только canonical UUID; workflows request assertions |
| sdlc question/revision publish | POST `/issues/{uuid}/sdlc/clarifications`, `/requirements` | JSON file/stdin body as-is; no idempotency header/retry; workflows |
| sdlc intent / analysis preflight | GET `/issues/{uuid}/sdlc/analysis-intent`, `/analysis-reservation/configuration-preflight` | canonical UUID; workflows |
| sdlc analysis reservation read/reserve/heartbeat/operation | GET/POST `/issues/{uuid}/sdlc/analysis-reservation`; POST `/heartbeat`; GET `/operations/{key}` | JSON body as-is; escaped operation key; workflows |
| sdlc analysis result submit/operation | POST `/issues/{uuid}/sdlc/analysis-result`; GET `/operations/{key}` | JSON file/stdin; readback without write retry; workflows |
| trash + issue restore | GET `/projects/{key}/trash`; POST `/issues/{id}/restore` | пагинация; real_api + application access test |

### Границы пакета

Нет новых команд управления определениями custom fields, components/versions, watchers/votes, export или безвозвратным удалением. SDLC packet не добавляет MCP/Fleet admission profile, новый runtime-протокол, proof endpoint, scheduler или server schema. Административные команды сохраняются на прежнем уровне. Тестовый real_api запускает настоящие handlers/application services с изолированными memory repositories и storage; это проверка CLI/API, а не приёмка PostgreSQL persistence.

```bash
cd backend
cargo test -p task-tracker-cli
cargo test -p app issue_identifier_resolves_key_uuid_and_restore_with_access_check
```

Справка показывает имена token env variables, скрывая их значения даже при установленной переменной.

Подтверждения project/issue/comment/label delete, label detach, notification read/read-all и member remove сохраняют прежний текст в `table` и `compact`. В `json` успешный пустой ответ выводится только как `{"status":"ok"}`. При ошибке API подтверждение успеха отсутствует.

## Готовые сборки

Установка, platform requirements, source/checksum и ограничения локального candidate: [CLI_INSTALL.md](CLI_INSTALL.md).

Текущие Linux/WSL candidates и установка: [CLI_INSTALL.md](CLI_INSTALL.md). Проверки против образов с PostgreSQL, границы fixture execution и блокеры обновления sdlc1: [CLI_VALIDATION.md](CLI_VALIDATION.md). Перед использованием с рабочим стендом требуется подтверждённая совместимость его backend и применённых миграций.
