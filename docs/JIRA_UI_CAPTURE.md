# Jira UI/UX Capture Report

## Source
- Instance: https://task.wemakedev.ru
- User: Александр Жуков (azhukov)
- Read-only access. **No data modified.**
- Captured: {
  "projects": len(jira["projects"]),
  "issue_types": len(jira["issue_types"]),
  "statuses": len(jira["statuses"]),
  "priorities": len(jira["priorities"]),
  "resolutions": len(jira["resolutions"]),
  "link_types": len(jira["link_types"]),
  "custom_fields": len(jira["custom_field_types"]),
  "system_fields": len(jira["system_field_names"]),
  "boards": len(jira["boards"]),
  "dashboards": len(jira["dashboards"])
}

## Projects
| ID | Key | Name | Type |
|---|---|---|---|
| 10102 | ED |  Education | software |
| 10103 | AAT | AZHUKOV Azhukov Test | software |
| 10001 | PRES | Presale | business |
| 10100 | PT | Product team | business |
| 10105 | ATS | АТС Релевантер | software |
| 10002 | AIPROJ | ИИ-проекты | software |
| 10000 | NEUROKEY | Нейроключ | software |
| 10106 | EDU | Обучения  | software |
| 10104 | REL | Релевантер | software |
| 10101 | NKTIME | Учет рабочего времени | business |

## Issue Types
| ID | Name | Subtask |
|---|---|---|
| 10100 | Задача | no |
| 10101 | Подзадача | yes |
| 10001 | История | no |
| 10102 | Ошибка | no |
| 10000 | Epic | no |
| 10200 | Маркетинг | no |
| 10202 | Продукт | no |
| 10201 | Продажи | no |
| 10203 | Администрирование | no |
| 10204 | Анализ | no |
| 10205 | Техническая | no |
| 10206 | Таймшит | no |

## Statuses
| ID | Name | Category | Category Key | Color |
|---|---|---|---|---|
| 1 | Открытый | К выполнению | new | default |
| 3 | В работе | В работе | indeterminate | inprogress |
| 10000 | Сделать | К выполнению | new | default |
| 10001 | Готово | Выполнено | done | success |
| 10002 | Backlog | К выполнению | new | default |
| 10003 | Selected for Development | К выполнению | new | default |
| 10100 | Тестирование | В работе | indeterminate | inprogress |
| 10201 | Review | В работе | indeterminate | inprogress |
| 10202 | Canceled | Выполнено | done | success |
| 10203 | Готово к тестированию | К выполнению | new | default |
| 10204 | Готово к Релизу | К выполнению | new | default |
| 10206 | Ревью | К выполнению | new | default |
| 10207 | В доработку | К выполнению | new | default |
| 10208 | Готовится КП (без маржи) | В работе | indeterminate | inprogress |
| 10209 | КП готово (без маржи) | В работе | indeterminate | inprogress |
| 10210 | Ожидает ОС (от клиента) | В работе | indeterminate | inprogress |
| 10211 | Согласовано | В работе | indeterminate | inprogress |
| 10212 | Пилот | В работе | indeterminate | inprogress |
| 10213 | Вопросы к заказчику | В работе | indeterminate | inprogress |
| 10214 | Выполнено | Выполнено | done | success |
| 10215 | КП готово( без маржи) | В работе | indeterminate | inprogress |
| 10216 | Ожидает ОС(от клиента) | В работе | indeterminate | inprogress |
| 10217 | Согласовано(с заказчиком) | В работе | indeterminate | inprogress |

## Priorities
| ID | Name |
|---|---|
| 1 | Highest |
| 2 | High |
| 3 | Medium |
| 4 | Low |
| 5 | Lowest |

## Resolutions
| ID | Name |
|---|---|
| 10000 | Готово |
| 10001 | Не получится |
| 10002 | Дубликат |
| 10003 | Не воспроизводится |
| 10004 | Готово |
| 10005 | Не будет выполнено |

## Link Types
| ID | Name | Inward | Outward |
|---|---|---|---|
| 10000 | Blocks | is blocked by | blocks |
| 10001 | Cloners | is cloned by | clones |
| 10002 | Duplicate | is duplicated by | duplicates |
| 10003 | Relates | relates to | relates to |

## System Fields
| ID | Name |
|---|---|
| issuetype | Тип задачи |
| timespent | Затраченное время |
| project | Проект |
| fixVersions | Исправить в версиях |
| aggregatetimespent | Суммарное затраченое время |
| resolution | Решение |
| resolutiondate | Дата решения |
| workratio | КПД (полезная работа/затраченная работа) |
| lastViewed | Последний просмотр |
| watches | Наблюдатели |
| thumbnail | Изображения |
| created | Создано |
| priority | Приоритет |
| labels | Метки |
| timeestimate | Оставшееся время |
| aggregatetimeoriginalestimate | Суммарная первоначальная оценка |
| versions | Затронуты версии |
| issuelinks | Связанные задачи |
| assignee | Исполнитель |
| updated | Обновленo |
| status | Статус |
| components | Компоненты |
| issuekey | Код |
| timeoriginalestimate | Первоначальная оценка |
| description | Описание |
| archiveddate | Дата добавления в архив |
| timetracking | Учет времени |
| security | Уровень безопасности |
| attachment | Вложение |
| aggregatetimeestimate | Суммарное оставшееся время |
| summary | Тема |
| creator | Создатель |
| subtasks | Подзадачи |
| reporter | Автор |
| aggregateprogress | Суммарный прогресс |
| environment | Окружение |
| duedate | Срок исполнения |
| progress | Прогресс |
| comment | Комментарий |
| votes | Голоса |
| worklog | Сделать запись о работе |
| archivedby | Кто добавил в архив |

## Custom Fields
| ID | Name | Type | Custom Type |
|---|---|---|---|
| customfield_10110 | Исторические точки | number | com.atlassian.jira.plugin.system.customfieldtypes:float |
| customfield_10111 | Команда | any | com.atlassian.teams:rm-teams-custom-field-team |
| customfield_10104 | Имя эпика | string | com.pyxis.greenhopper.jira:gh-epic-label |
| customfield_10105 | Статус эпика | option | com.pyxis.greenhopper.jira:gh-epic-status |
| customfield_10106 | Цвет эпика | string | com.pyxis.greenhopper.jira:gh-epic-color |
| customfield_10107 | Ссылка на эпик | any | com.pyxis.greenhopper.jira:gh-epic-link |
| customfield_10108 | Спринт | array | com.pyxis.greenhopper.jira:gh-sprint |
| customfield_10109 | Рейтинг | any | com.pyxis.greenhopper.jira:gh-lexo-rank |
| customfield_10100 | Родительская ссылка | any | com.atlassian.jpo:jpo-custom-field-parent |
| customfield_10101 | Целевая дата начала | date | com.atlassian.jpo:jpo-custom-field-baseline-start |
| customfield_10102 | Целевая дата окончания | date | com.atlassian.jpo:jpo-custom-field-baseline-end |
| customfield_10103 | Первоначальная оценка сложности | number | com.atlassian.jpo:jpo-custom-field-original-story-points |
| customfield_10000 | Development | any | com.atlassian.jira.plugins.jira-development-integration-plugin:devsummary |
| customfield_10200 | Отмечено | array | com.atlassian.jira.plugin.system.customfieldtypes:multicheckboxes |
| customfield_10201 | Пойнты | number | com.atlassian.jira.plugin.system.customfieldtypes:float |
| customfield_10202 | Оценка | number | com.atlassian.jira.plugin.system.customfieldtypes:float |

## Boards
| ID | Name | Type |
|---|---|---|
| 10 | Paper1 | kanban |
| 2 | Presale | kanban |
| 4 | Product Team | kanban |
| 3 | Доска AIPROJ | kanban |
| 12 | Доска ATS | kanban |
| 1 | Доска NEUROKEY | scrum |
| 11 | Доска Релевантер | kanban |
| 13 | Обучения | kanban |

## Board Columns

### Board 10 — Paper1
- **Список задач** (—)
- **Нужно сделать** (—)
- **В работе** (—)
- **Выполнено** (—)

### Board 2 — Presale
- **Список задач** (—)
- **Нужно сделать** (—)
- **Готовится КП (Без маржи)** (—)
- **Есть вопросы (к заказчику)** (—)
- **КП Готово (Без маржи)** (—)
- **Ожидает ОС (от клиента)** (—)
- **Согласовано (с заказчиком)** (—)
- **Пилот** (—)
- **Выполнено** (—)

### Board 4 — Product Team
- **Список задач** (—)
- **Backlog / Ideas** (—)
- **To Do** (—)
- **In Progress** (—)
- **Review** (—)
- **Done** (—)
- **Canceled** (—)

### Board 3 — Доска AIPROJ
- **Список задач** (—)
- **Список задач** (—)
- **Выбрано для разработки** (—)
- **В работе** (—)
- **Выполнено** (—)

### Board 12 — Доска ATS
- **Список задач** (—)
- **Список задач** (—)
- **Выбрано для разработки** (—)
- **В работе** (—)
- **Ревью** (—)
- **Тестирование** (—)
- **Выполнено** (—)

### Board 1 — Доска NEUROKEY
- **Нужно сделать** (—)
- **В работе** (—)
- **Ревью** (—)
- **Готово к тестированию** (—)
- **Готово к Релизу** (—)
- **Выполнено** (—)

### Board 11 — Доска Релевантер
- **Список задач** (—)
- **Список задач** (—)
- **Выбрано для разработки** (—)
- **В работе** (—)
- **Ревью** (—)
- **Тестирование** (—)
- **Выполнено** (—)

### Board 13 — Обучения
- **Список задач** (—)
- **Идеи ** (—)
- **В разработке (Программа)** (—)
- **Согласование / Ревью ** (—)
- **Готово к запуску** (—)
- **Набор / Маркетинг** (—)
- **В процессе (Обучение)** (—)
- **Завершено** (—)

## Key UI Insights

1. **Status category colors**: default (new/To Do), inprogress (indeterminate), success (done).
2. **Kanban board structure**: always has a backlog/list column + status columns.
3. **Issue detail fields observed**: summary, description, assignee, reporter, priority, status, labels, components, fixVersions, versions, timetracking, attachment, comment, worklog, issuelinks, subtasks, votes, watches, creator, created, updated, duedate, environment, progress.
4. **Custom fields present**: epic fields, sprint, story points, original estimate, team.
5. **Attachments include**: filename, mimeType, size, content URL, thumbnail URL, author, created.
6. **Comments include**: id, author, body, created, updated.
7. **Dashboard**: only System Dashboard visible via API; gadgets need gadget API.

## Files Captured
- `docs/assets/jira-reference/jira-reference-data.json` — full raw API responses.
- `docs/assets/jira-reference/jira-ui-ux-summary.json` — condensed summary.
- `docs/assets/jira-reference/jira_issue_*.json` — sample issue payloads with comments/attachments.
- `docs/assets/ui-mockups/*.html` — HTML mockups for React implementation.
