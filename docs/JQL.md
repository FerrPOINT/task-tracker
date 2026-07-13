# JQL (Jira Query Language) Specification

## 1. Общее

JQL — язык поиска задач. Поддерживает поля, операторы, функции, логические операторы, скобки.

---

## 2. Синтаксис (EBNF)

```ebnf
query          := clause (logical_op clause)*
clause         := atomic_clause
                | "(" query ")"
                | "NOT" clause

atomic_clause  := field op value
                | field "IN" value_list
                | field "NOT" "IN" value_list
                | field "IS" "EMPTY"
                | field "IS" "NOT" "EMPTY"
                | field "~" string_value
                | field "!~" string_value
                | field "CHANGED"
                | field "CHANGED" "TO" value (time_condition)?
                | field "CHANGED" "FROM" value (time_condition)?
                | field "WAS" value (time_condition)?
                | field "WAS" "IN" value_list (time_condition)?
                | field "WAS" "NOT" value (time_condition)?
                | field "WAS" "NOT" "IN" value_list (time_condition)?

time_condition := "AFTER" datetime_value
                | "BEFORE" datetime_value
                | "ON" date_value
                | "DURING" "(" datetime_value "," datetime_value ")"

logical_op     := "AND" | "OR"

field          := system_field | custom_field
system_field   := "key" | "summary" | "description" | "status" | "statusCategory"
                | "project" | "projectKey" | "issueType" | "assignee" | "reporter"
                | "creator" | "priority" | "resolution" | "resolutionDate"
                | "created" | "updated" | "dueDate" | "startDate"
                | "labels" | "components" | "fixVersion" | "affectedVersion"
                | "epic" | "parent" | "sprint" | "text" | "timeSpent"
                | "originalEstimate" | "remainingEstimate" | "votes" | "watchers"
                | "cf" [custom field id]

custom_field   := "cf_" uuid | '"' custom field name '"'

op             := "=" | "!=" | "<" | "<=" | ">" | ">="

value          := string_value | number_value | date_value | datetime_value
                | user_value | project_value | version_value | function_value

value_list     := "(" value ("," value)* ")"

string_value   := '"' string '"' | unquoted_string
number_value   := integer | decimal
date_value     := '"' YYYY-MM-DD '"'
datetime_value := '"' ISO8601 '"'
user_value     := string_value | function_value
project_value  := string_value | function_value
version_value  := string_value
function_value := function_name "(" args? ")"
function_name  := "currentUser" | "now" | "startOfDay" | "startOfWeek"
                | "startOfMonth" | "startOfYear" | "endOfDay" | "endOfWeek"
                | "endOfMonth" | "endOfYear" | "membersOf" | "projectMatch"
                | "issueHistory" | "sprintInOpenSprints" | "sprintInClosedSprints"
args           := string_value ("," string_value)*
```

---

## 3. Системные поля

| Поле | Тип | Описание |
|------|-----|----------|
| key | string | `TT-42` |
| summary | string | Заголовок |
| description | text | Full-text |
| status | string | Имя или ID статуса |
| statusCategory | string | todo/in_progress/done |
| project | string | Имя или ID проекта |
| projectKey | string | `TT` |
| issueType | string | Имя или ID типа задачи |
| assignee | user | username / id / currentUser() |
| reporter | user | — |
| creator | user | — |
| priority | string | highest/high/medium/low/lowest |
| resolution | string | fixed/wontfix/duplicate/...
| resolutionDate | datetime | — |
| created | datetime | — |
| updated | datetime | — |
| dueDate | date | — |
| startDate | date | — |
| labels | string[] | — |
| components | string[] | — |
| fixVersion | string | — |
| affectedVersion | string | — |
| epic | string | key или ID epic |
| parent | string | key или ID parent |
| sprint | string | имя или ID sprint |
| text | text | Full-text по summary+description+comments |
| timeSpent | seconds | — |
| originalEstimate | seconds | — |
| remainingEstimate | seconds | — |
| votes | integer | — |
| watchers | integer | — |

---

## 4. Операторы

| Оператор | Описание | Пример |
|----------|----------|--------|
| = | равно | `status = "In Progress"` |
| != | не равно | `assignee != currentUser()` |
| < | меньше | `created < 2026-01-01` |
| <= | меньше или равно | `dueDate <= endOfWeek()` |
| > | больше | `votes > 5` |
| >= | больше или равно | `timeSpent >= 3600` |
| ~ | contains (full text) | `summary ~ "auth"` |
| !~ | not contains | `description !~ "deprecated"` |
| IN | в списке | `status IN ("To Do", "In Progress")` |
| NOT IN | не в списке | `project NOT IN ("ARCH", "LEGACY")` |
| IS EMPTY | пусто | `assignee IS EMPTY` |
| IS NOT EMPTY | не пусто | `fixVersion IS NOT EMPTY` |
| WAS | был в статусе | `status WAS "In Progress"` |
| WAS IN | был в одном из | `status WAS IN ("To Do", "In Progress")` |
| CHANGED | изменялось | `status CHANGED` |
| CHANGED TO | изменилось в | `status CHANGED TO "Done" AFTER startOfWeek()` |

---

## 5. Логические операторы

- `AND` — пересечение
- `OR` — объединение
- `NOT` — отрицание
- Скобки для управления приоритетом

Приоритет: `NOT` > `AND` > `OR`.

---

## 6. Функции

| Функция | Описание | Пример |
|---------|----------|--------|
| currentUser() | Текущий пользователь | `assignee = currentUser()` |
| now() | Текущий timestamp | `updated < now()` |
| startOfDay() | 00:00 сегодня | `created >= startOfDay()` |
| startOfWeek() | Начало недели | `created >= startOfWeek()` |
| startOfMonth() | Начало месяца | — |
| startOfYear() | Начало года | — |
| endOfDay() | Конец дня | — |
| endOfWeek() | Конец недели | — |
| endOfMonth() | — | — |
| endOfYear() | — | — |
| membersOf("role") | Пользователи в роли | `assignee IN membersOf("developers")` |
| projectMatch("regex") | Проекты по regexp | `project IN projectMatch("TT.*")` |
| issueHistory() | Задачи из истории пользователя | `key IN issueHistory()` |
| sprintInOpenSprints() | Задачи в активных спринтах | `sprint IN openSprints()` |
| openSprints() | Активные спринты | `sprint IN openSprints()` |
| closedSprints() | Закрытые спринты | — |

---

## 7. Примеры запросов

### Простые

```sql
project = TT
project = TT AND status = "To Do"
assignee = currentUser()
status != Done
priority IN (High, Highest)
```

### Поиск по тексту

```sql
text ~ "authentication"
summary ~ "login"
description ~ "oauth"
```

### Временные

```sql
created >= startOfWeek()
updated >= -1d
dueDate <= endOfWeek()
resolutionDate >= 2026-01-01
```

### Пользователи и роли

```sql
assignee IN membersOf("developers")
reporter = currentUser()
assignee IS EMPTY
assignee != reporter
```

### Версии и релизы

```sql
fixVersion = "v1.0.0"
affectedVersion IS NOT EMPTY
fixVersion IN ("v1.0.0", "v1.1.0")
```

### Epic и hierarchy

```sql
epic = "TT-1"
parent = "TT-10"
issueType = Sub-task
issueType != Epic
```

### Спринты

```sql
sprint = "Sprint 1"
sprint IN openSprints()
sprint IN closedSprints()
```

### Labels / components

```sql
labels = backend
labels IN (backend, frontend)
components = "API"
```

### Сложные

```sql
project = TT AND (assignee = currentUser() OR reporter = currentUser()) AND status != Done
project = TT AND status IN ("To Do", "In Progress") AND priority IN (High, Highest) AND created >= startOfWeek()
text ~ "bug" AND project IN (TT, DEV) AND statusCategory != done
```

### История

```sql
status WAS "In Progress" AND status = "Done"
status CHANGED TO "Done" AFTER startOfWeek()
assignee CHANGED FROM "jdoe" TO "asmith"
```

---

## 8. Относительные даты

| Синтаксис | Описание |
|-----------|----------|
| `-1d` | 1 день назад |
| `-2w` | 2 недели назад |
| `-3m` | 3 месяца назад |
| `-1y` | 1 год назад |
| `+1d` | через 1 день |

---

## 9. Преобразование в SQL

JQL AST → SQL builder:

```sql
-- project = TT AND status != Done
SELECT i.* FROM issues i
JOIN projects p ON i.project_id = p.id
WHERE p.key = 'TT' AND i.deleted_at IS NULL
  AND i.status_id NOT IN (SELECT id FROM statuses WHERE category = 'done');

-- text ~ 'auth'
SELECT i.* FROM issues i
WHERE i.tsv_search @@ plainto_tsquery('auth');

-- created >= startOfWeek()
WHERE i.created_at >= date_trunc('week', now());
```

---

## 10. Валидация JQL

- Проверка существования полей.
- Проверка типов значений.
- Проверка прав доступа к полям (security level).
- Лимит сложности запроса: max 50 clauses.

---

## 11. Сохранённые фильтры

Пользователь сохраняет JQL как `SavedFilter`. Поддерживается:
- личные фильтры
- публичные фильтры
- подписка на фильтр (email digest)
- использование в dashboard gadgets

---

## 12. API

| Метод | Endpoint | Описание |
|-------|----------|----------|
| GET | /issues?jql=... | Выполнить JQL |
| POST | /filters | Сохранить фильтр |
| GET | /filters/{id}/execute | Выполнить сохранённый фильтр |
| POST | /jql/validate | Проверить синтаксис |
| POST | /jql/parse | Вернуть AST |
## References

- `docs/DATA_MODEL.md`
- `docs/API.md`
- `docs/USER_STORIES.md`
