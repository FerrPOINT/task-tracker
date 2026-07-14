# CLI Specification — Task Tracker

## 1. Overview

CLI — отдельный Rust crate `cli/`. Предоставляет инструменты для администрирования, миграций, backup/restore, импорта и диагностики.

## 2. Installation

```bash
cargo install --path cli
task-tracker --version
```

## 3. Global Flags

| Flag | Description |
|------|-------------|
| `--api-url` | Базовый URL API (`http://localhost:19876`) |
| `--token` | JWT access token |
| `--config` | Путь к config файлу |
| `-v, --verbose` | Подробный вывод |
| `-h, --help` | Справка |

## 4. Authentication Commands

### 4.1 Login

```bash
task-tracker auth login --username alice --password-stdin
task-tracker auth login --token $TASKTRACKER_TOKEN
task-tracker auth whoami
task-tracker auth logout
```

### 4.2 Token Storage

- Токен сохраняется в `~/.config/task-tracker/credentials.json` с permissions 0600.
- Refresh cookie не сохраняется в CLI.

## 5. Project Commands

### 5.1 CRUD

```bash
task-tracker project list
task-tracker project create --key PROJ --name "Project Name" --type software --lead alice
task-tracker project get PROJ
task-tracker project update PROJ --name "New Name"
task-tracker project delete PROJ --force
```

### 5.2 Roles

```bash
task-tracker project role add PROJ --user alice --role "Project Admin"
task-tracker project role remove PROJ --user alice --role "Developer"
task-tracker project role list PROJ
```

## 6. Issue Commands

```bash
task-tracker issue create --project PROJ --type Task --summary "Fix bug" --assignee alice
task-tracker issue get PROJ-123
task-tracker issue update PROJ-123 --status "In Progress"
task-tracker issue delete PROJ-123
task-tracker issue search --jql "project = PROJ AND status = Done"
task-tracker issue transition PROJ-123 --to Done
```

## 7. User Commands

```bash
task-tracker user list
task-tracker user create --username alice --email alice@example.com --display-name "Alice"
task-tracker user get alice
task-tracker user deactivate alice
task-tracker user group add alice jira-administrators
```

## 8. Import / Export

### 8.1 CSV Import

```bash
task-tracker import csv issues.csv --project PROJ --mapping mapping.json --dry-run
task-tracker import csv issues.csv --project PROJ --execute
```

### 8.2 JSON Export

```bash
task-tracker export project PROJ --output proj-backup.json
task-tracker export issues --jql "project = PROJ" --output issues.json
```

### 8.3 Restore

```bash
task-tracker restore proj-backup.json --mode merge
task-tracker restore proj-backup.json --mode replace
```

## 9. Migration Commands

```bash
task-tracker migrate status
task-tracker migrate up
task-tracker migrate down --count 1
task-tracker migrate redo
task-tracker migrate create add_issue_types_table
```

## 10. Backup / Restore

```bash
task-tracker backup create --output /backups/tt-$(date +%F).sql
task-tracker backup list
task-tracker backup restore /backups/tt-2026-07-13.sql
```

## 11. Admin Commands

```bash
task-tracker admin reindex
task-tracker admin audit --user alice --from 2026-07-01
task-tracker admin settings get smtp.host
task-tracker admin settings set smtp.host smtp.example.com
```

## 12. Server Commands

```bash
task-tracker server health
task-tracker server metrics
task-tracker server logs --follow
```

## 13. Configuration File

### 13.1 Path

`~/.config/task-tracker/config.toml`

### 13.2 Example

```toml
[api]
url = "http://localhost:19876"
timeout_seconds = 30

[auth]
token_file = "~/.config/task-tracker/credentials.json"

[output]
format = "table"  # table | json | yaml
```

## 14. Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Authentication failed |
| 4 | Permission denied |
| 5 | Resource not found |
| 6 | API error |
| 7 | Network error |

## 15. Output Formats

```bash
task-tracker project list --output json
task-tracker project list --output yaml
task-tracker project list --output table
```

## 16. Shell Completions

```bash
task-tracker completions bash > /etc/bash_completion.d/task-tracker
task-tracker completions zsh > /usr/local/share/zsh/site-functions/_task-tracker
task-tracker completions fish > ~/.config/fish/completions/task-tracker.fish
```
## References

- `docs/ARCHITECTURE.md`
- `docs/API.md`
- `docs/DEPLOYMENT.md`
