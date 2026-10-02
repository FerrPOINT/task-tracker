# CLI — Task Tracker

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
- HTTP-транспорт предоставляется `sdlc-cli-core` из соседнего `services-base`; удалённые URL требуют HTTPS.
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
task-tracker issue delete PROJ-1
task-tracker trash --project-key PROJ --limit 25 --offset 0
task-tracker issue restore PROJ-1
```

Issue filters: `--q`, `--priority`, `--status`, `--assignee-id`, `--sort-by`, `--sort-order`, `--limit`, `--offset`. Сервер валидирует значения сортировки и диапазоны. `search global`, `search jql`, `board backlog`, `notification list`, `comment list` принимают `--limit/--offset`; `notification list --include-read` включает прочитанные. `transitions` показывает переходы из текущего статуса, не обещая разрешение перехода: окончательная валидация выполняется сервером. `field set` принимает JSON, включая `null`, а не неявно преобразованную строку.

MIME type upload определяется по имени файла; `--content-type` переопределяет его. Сервер сохраняет свой allowlist допустимых типов.

`--unassign` передаёт JSON `null` и конфликтует с `--assignee-id`. Для comment add/update поддержаны прежний inline body и `--from-file`. Форматы `json|table|compact` сохранены; default `json`. Ошибки выполнения возвращают прежний код `1`, ошибки CLI parsing — `2`.

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
| trash + issue restore | GET `/projects/{key}/trash`; POST `/issues/{id}/restore` | пагинация; real_api + application access test |

### Границы пакета

Нет новых команд управления определениями custom fields, components/versions, watchers/votes, export или безвозвратным удалением. Административные команды сохраняются на прежнем уровне. Тестовый real_api запускает настоящие handlers/application services с изолированными memory repositories и storage; это проверка CLI/API, а не приёмка PostgreSQL persistence.

```bash
cd backend
cargo test -p task-tracker-cli
cargo test -p app issue_identifier_resolves_key_uuid_and_restore_with_access_check
```

Справка показывает имена token env variables, скрывая их значения даже при установленной переменной.

Подтверждения project/issue/comment/label delete, label detach, notification read/read-all и member remove сохраняют прежний текст в `table` и `compact`. В `json` успешный пустой ответ выводится только как `{"status":"ok"}`. При ошибке API подтверждение успеха отсутствует.

## Готовые сборки

Установка, platform requirements, source/checksum и ограничения локального candidate: [CLI_INSTALL.md](CLI_INSTALL.md).
