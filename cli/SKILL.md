---
name: task-tracker-cli
description: Manage Task Tracker projects, issues, boards, sprints and more via CLI. Use when you need to create, read, update, or delete tasks/projects/issues through the Task Tracker API.
---

# Task Tracker CLI

CLI для управления Task Tracker через API. Бинарник: `task-tracker` (Rust, собирается из `backend/cli/`).

## Сборка

```bash
cd /opt/dev/task-tracker/backend && cargo build --bin task-tracker
# или: cargo install --path backend/cli
```

## Конфигурация

Переменные окружения (или флаги):

| Env | Flag | Default | Описание |
|-----|------|---------|----------|
| `TASKTRACKER_API_URL` | `--api-url` | `http://localhost:19876` | URL API |
| `TASKTRACKER_TOKEN` | `--token` | — | Bearer JWT токен |
| `TASKTRACKER_OUTPUT` | `--output` | `json` | Формат вывода: `json` \| `table` \| `compact` |

## Аутентификация

```bash
# Регистрация
task-tracker auth register --email admin@example.com --username admin --display-name Admin --password secret

# Логин (вернёт access_token — сохрани в TASKTRACKER_TOKEN)
task-tracker auth login --email admin@example.com --password secret

# Текущий пользователь
task-tracker auth whoami
```

## Проекты

```bash
task-tracker project list
task-tracker project create --key TT --name "Test Project"
task-tracker project get TT
task-tracker project update TT --name "Renamed"
task-tracker project delete TT
```

## Задачи (Issues)

```bash
# Создать задачу
task-tracker issue create --project-key TT --summary "Fix bug" --priority high

# Получить задачу по ID
task-tracker issue get <issue-id>

# Обновить
task-tracker issue update <issue-id> --summary "Fixed" --status-id <status-uuid>

# Удалить (soft-delete)
task-tracker issue delete <issue-id>

# Сменить статус
task-tracker issue transition <issue-id> --to <status-uuid>

# Список задач проекта
task-tracker issue list --project-key TT
```

## Доска (Kanban)

```bash
task-tracker board get --project-key TT
task-tracker board backlog --project-key TT
task-tracker board move --project-key TT --issue-id <id> --status-id <status-uuid>
```

## Спринты

```bash
task-tracker sprint list --project-key TT
task-tracker sprint create --project-key TT --name "Sprint 1"
task-tracker sprint start <sprint-id>
task-tracker sprint close <sprint-id>
task-tracker sprint add-issue --sprint-id <id> --issue-id <issue-id>
task-tracker sprint remove-issue --sprint-id <id> --issue-id <issue-id>
```

## Комментарии

```bash
task-tracker comment list <issue-id>
task-tracker comment add --issue-id <issue-id> --body "Looks good"
task-tracker comment update --comment-id <id> --body "Updated"
task-tracker comment delete <comment-id>
```

## Метки (Labels)

```bash
task-tracker label list TT
task-tracker label create --project-key TT --name bug --color red
task-tracker label delete <label-id>
task-tracker label attach --issue-id <id> --label-id <label-id>
task-tracker label detach --issue-id <id> --label-id <label-id>
```

## Поиск

```bash
# Глобальный поиск
task-tracker search global --q "bug" --project-key TT

# JQL запрос
task-tracker search jql 'project = "TT" AND status = "Open" ORDER BY priority DESC'
```

## Уведомления

```bash
task-tracker notification list
task-tracker notification read <notification-id>
task-tracker notification read-all
task-tracker notification settings
task-tracker notification update-settings --email-frequency immediate
```

## Отчёты

```bash
task-tracker report velocity --project-key TT --count 5
task-tracker report burndown --sprint-id <id>
task-tracker report cumulative-flow --project-key TT
task-tracker report control-chart --project-key TT
```

## Админка

```bash
task-tracker admin list-users
task-tracker admin create-user --email user@example.com --username user --display-name User --password pass
task-tracker admin toggle-user <user-id> --active false
task-tracker admin audit-log --limit 20
task-tracker admin settings
task-tracker admin set-setting --key site.name --value "My Tracker"
```

## Участники проекта

```bash
task-tracker member list TT
task-tracker member add --project-key TT --user-id <uuid> --role member
task-tracker member remove --project-key TT --user-id <uuid>
```

## Форматы вывода

- **json** (по умолчанию) — полный JSON ответ API
- **compact** — `id | key | name | status` для списков
- **table** — TSV (tab-separated) для piping в другие инструменты

```bash
task-tracker --output compact project list
task-tracker --output table issue list --project-key TT
```

## Типовой workflow для ИИ

```bash
# 1. Логин
TOKEN=$(task-tracker auth login --email admin@example.com --password secret | jq -r '.access_token')
export TASKTRACKER_TOKEN=$TOKEN

# 2. Создать проект
task-tracker project create --key PROJ --name "New Project"

# 3. Создать задачи
task-tracker issue create --project-key PROJ --summary "Task 1" --priority high
task-tracker issue create --project-key PROJ --summary "Task 2" --priority medium

# 4. Посмотреть доску
task-tracker board get --project-key PROJ

# 5. Создать спринт
task-tracker sprint create --project-key PROJ --name "Sprint 1"

# 6. Комментарий
task-tracker comment add --issue-id <issue-id> --body "Started work"
```