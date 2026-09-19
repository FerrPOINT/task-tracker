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
- 12 групп команд полностью реализованы: auth, project, issue, board, sprint, comment, label, search, notification, report, admin, member.

## Ссылки

- `docs/API.md`
- `docs/ARCHITECTURE.md`

## References

- [ARCHITECTURE](ARCHITECTURE.md)
- [LOCAL_SETUP](LOCAL_SETUP.md)
