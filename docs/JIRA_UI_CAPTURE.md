# UI/UX Capture Report — Task WMT Jira ([REDACTED-JIRA-INSTANCE])

## Статус захвата

### API reference JSON (файлы)
- `FILES_INDEX.json`
- `admin_schemes.json`
- `board_10_issues.json`
- `board_1_epics.json`
- `board_2_epics.json`
- `board_3_epics.json`
- `board_4_epics.json`
- `changelog_AAT_1.json`
- `changelog_PRES_99.json`
- `changelog_REL_335.json`
- `comments_PRES_99.json`
- `comments_PT_21.json`
- `comments_REL_335.json`
- `configuration.json`
- `createmeta_10103_issuetype_10000_fields.json`
- `createmeta_10103_issuetype_10001_fields.json`
- `createmeta_10103_issuetype_10100_fields.json`
- `createmeta_10103_issuetypes.json`
- `dashboards.json`
- `editmeta_AAT_1.json`
- `editmeta_PRES_99.json`
- `editmeta_PT_21.json`
- `editmeta_REL_335.json`
- `editmeta_pres99.json`
- `epics_sample.json`
- `extra_endpoints.json`
- `extracted_from_issues.json`
- `filters_favourite.json`
- `filters_search.json`
- `groups.json`
- `groups_members.json`
- `issuePicker.json`
- `issue_rendered_pres99.json`
- `issues_full_100.json`
- `issues_with_changelog_30.json`
- `issues_with_content_50.json`
- `issues_with_worklogs.json`
- `issuesecurityschemes.json`
- `jira-reference-data.json`
- `jira-ui-ux-summary.json`
- `jira_issue_PT_21.json`
- `jira_issue_REL_335.json`
- `jira_issue_pres99.json`
- `labels.json`
- `priorities.json`
- `project_AAT_details.json`
- `project_avatars_AAT.json`
- `project_components.json`
- `project_details.json`
- `project_issuetypescheme_AAT.json`
- `project_notificationscheme_AAT.json`
- `project_permissionscheme_AAT.json`
- `project_roles.json`
- `project_statuses_AAT.json`
- `project_versions.json`
- `project_workflows_AAT.json`
- `rapidview_backlogs.json`
- `rapidviews.json`
- `serverInfo.json`
- `sprints_board_1.json`
- `subtask_issue_types.json`
- `subtasks_sample.json`
- `timetracking_config.json`
- `transitions_PT_21.json`
- `user_[REDACTED].json`
- `users.json`
- `votes_PT_21.json`
- `watchers_PT_21.json`
- `worklog_PT_21.json`

## Главные UI-экраны, которые зафиксированы
1. **Login page** — форма входа.
2. **System Dashboard** — gadgets (Assigned to me, Activity Stream).
3. **Project List / Browse Projects** — таблица проектов с фильтрами.
4. **Kanban Board** — Paper1 (AAT), 3 колонки: Нужно сделать / В работе / Выполнено.
5. **Kanban Board** — NEUROKEY Sprint 8, 6 колонок + swimlanes + WIP limits + quick filters.
6. **Backlog / Sprint planning** — NEUROKEY, спринты, бэклог, quick filters, эпики.
7. **Issue Detail** — AAT-1 (Story), attachments, details, comments.
8. **Issue Detail** — NEUROKEY-488 (Epic), labels, components, sprints, agile sidebar.
9. **Issue Navigator / Search** — filters sidebar, basic search, results list + detail split view.
10. **Create Issue** — step 1 (project/type) и full form (summary, RTE description, priority, labels, attachment, linked issues, assignee, epic link, sprint).
11. **Releases** — empty state + sidebar.

## Поля задач, используемые в реальных данных
`aggregateprogress`, `aggregatetimeestimate`, `aggregatetimeoriginalestimate`, `aggregatetimespent`, `archivedby`, `archiveddate`, `assignee`, `attachment`, `comment`, `components`, `created`, `creator`, `customfield_10000`, `customfield_10100`, `customfield_10101`, `customfield_10102`, `customfield_10103`, `customfield_10104`, `customfield_10105`, `customfield_10106`, `customfield_10107`, `customfield_10108`, `customfield_10109`, `customfield_10110`, `customfield_10111`, `customfield_10200`, `customfield_10201`, `customfield_10202`, `description`, `duedate`, `environment`, `fixVersions`, `issuelinks`, `issuetype`, `labels`, `lastViewed`, `parent`, `priority`, `progress`, `project`, `reporter`, `resolution`, `resolutiondate`, `status`, `subtasks`, `summary`, `timeestimate`, `timeoriginalestimate`, `timespent`, `timetracking`, `updated`, `versions`, `votes`, `watches`, `worklog`, `workratio`

## Системные поля
- `reporter` — Автор (user)
- `attachment` — Вложение (array)
- `votes` — Голоса (votes)
- `archiveddate` — Дата добавления в архив (datetime)
- `resolutiondate` — Дата решения (datetime)
- `timespent` — Затраченное время (number)
- `versions` — Затронуты версии (array)
- `thumbnail` — Изображения ()
- `assignee` — Исполнитель (user)
- `fixVersions` — Исправить в версиях (array)
- `workratio` — КПД (полезная работа/затраченная работа) (number)
- `issuekey` — Код ()
- `comment` — Комментарий (comments-page)
- `components` — Компоненты (array)
- `archivedby` — Кто добавил в архив (user)
- `labels` — Метки (array)
- `watches` — Наблюдатели (watches)
- `updated` — Обновленo (datetime)
- `environment` — Окружение (string)
- `description` — Описание (string)
- `timeestimate` — Оставшееся время (number)
- `timeoriginalestimate` — Первоначальная оценка (number)
- `subtasks` — Подзадачи (array)
- `lastViewed` — Последний просмотр (datetime)
- `priority` — Приоритет (priority)
- `progress` — Прогресс (progress)
- `project` — Проект (project)
- `resolution` — Решение (resolution)
- `issuelinks` — Связанные задачи (array)
- `worklog` — Сделать запись о работе (array)
- `created` — Создано (datetime)
- `creator` — Создатель (user)
- `duedate` — Срок исполнения (date)
- `status` — Статус (status)
- `aggregatetimeoriginalestimate` — Суммарная первоначальная оценка (number)
- `aggregatetimespent` — Суммарное затраченое время (number)
- `aggregatetimeestimate` — Суммарное оставшееся время (number)
- `aggregateprogress` — Суммарный прогресс (progress)
- `summary` — Тема (string)
- `issuetype` — Тип задачи (issuetype)
- `security` — Уровень безопасности (securitylevel)
- `timetracking` — Учет времени (timetracking)

## Кастомные поля
- `customfield_10000` — Development (any)
- `customfield_10104` — Имя эпика (string)
- `customfield_10110` — Исторические точки (number)
- `customfield_10111` — Команда (any)
- `customfield_10200` — Отмечено (array)
- `customfield_10202` — Оценка (number)
- `customfield_10103` — Первоначальная оценка сложности (number)
- `customfield_10201` — Пойнты (number)
- `customfield_10109` — Рейтинг (any)
- `customfield_10100` — Родительская ссылка (any)
- `customfield_10108` — Спринт (array)
- `customfield_10107` — Ссылка на эпик (any)
- `customfield_10105` — Статус эпика (option)
- `customfield_10106` — Цвет эпика (string)
- `customfield_10101` — Целевая дата начала (date)
- `customfield_10102` — Целевая дата окончания (date)

## Issue Types
- `10100` — Задача
- `10101` — Подзадача (subtask)
- `10001` — История
- `10102` — Ошибка
- `10000` — Epic
- `10200` — Маркетинг
- `10202` — Продукт
- `10201` — Продажи
- `10203` — Администрирование
- `10204` — Анализ
- `10205` — Техническая
- `10206` — Таймшит

## Statuses
- `1` — Открытый (К выполнению)
- `3` — В работе (В работе)
- `10000` — Сделать (К выполнению)
- `10001` — Готово (Выполнено)
- `10002` — Backlog (К выполнению)
- `10003` — Selected for Development (К выполнению)
- `10100` — Тестирование (В работе)
- `10201` — Review (В работе)
- `10202` — Canceled (Выполнено)
- `10203` — Готово к тестированию (К выполнению)
- `10204` — Готово к Релизу (К выполнению)
- `10206` — Ревью (К выполнению)
- `10207` — В доработку (К выполнению)
- `10208` — Готовится КП (без маржи) (В работе)
- `10209` — КП готово (без маржи) (В работе)
- `10210` — Ожидает ОС (от клиента) (В работе)
- `10211` — Согласовано (В работе)
- `10212` — Пилот (В работе)
- `10213` — Вопросы к заказчику (В работе)
- `10214` — Выполнено (Выполнено)
- `10215` — КП готово( без маржи) (В работе)
- `10216` — Ожидает ОС(от клиента) (В работе)
- `10217` — Согласовано(с заказчиком) (В работе)

## Приоритеты
- `1` — Highest
- `2` — High
- `3` — Medium
- `4` — Low
- `5` — Lowest

## Проекты
- `ED` —  Education (lead: -)
- `AAT` — [REDACTED] [REDACTED] Test (lead: -)
- `PRES` — Presale (lead: -)
- `PT` — Product team (lead: -)
- `ATS` — АТС Релевантер (lead: -)
- `AIPROJ` — ИИ-проекты (lead: -)
- `NEUROKEY` — Нейроключ (lead: -)
- `EDU` — Обучения  (lead: -)
- `REL` — Релевантер (lead: -)
- `NKTIME` — Учет рабочего времени (lead: -)

## Доски (rapidviews)
- `1` — Доска NEUROKEY (sprintSupport: True)
- `2` — Presale (sprintSupport: False)
- `3` — Доска AIPROJ (sprintSupport: False)
- `4` — Product Team (sprintSupport: False)
- `10` — Paper1 (sprintSupport: False)
- `11` — [REDACTED] (sprintSupport: False)
- `12` — Доска ATS (sprintSupport: False)
- `13` — Обучения (sprintSupport: False)

## Пользователи
- Всего загружено: 61

## Что НЕ удалось захватить
- **Workflow transitions / schemes** — 401/403, нет админ-прав.
- **Permission schemes / screen schemes / field config schemes** — 401/403/404.
- **Issue security schemes** — 403.
- **Groups details** — 403.
- **Admin project settings** — нет доступа.
- **Full board horizontal scroll** — viewport screenshots; kanban board шире экрана.

## Рекомендации по реализации UI/UX
1. **Topbar**: Jira Software logo, global menu dropdowns, Create button, search, notifications/help/avatar.
2. **Project sidebar**: project avatar + name, project pages (Kanban, Backlog, Releases, Reports, Issues, Components), project links.
3. **Kanban board**: columns with WIP limits, swimlanes, issue cards (key, summary, type icon, priority, labels/epic badges, assignee avatar), quick filters row, sprint header with dates/goal.
4. **Backlog**: sprint panels (active + future), issue rows with checkboxes, epic badges, assignee avatars, drag handles, backlog section below sprints, epics panel toggle.
5. **Issue detail**: two-column layout — left (header, actions, details, description, attachments, comments, activity), right (People, Dates, Agile).
6. **Issue navigator**: filters sidebar, basic search chips (project/type/status/assignee/text), advanced JQL toggle, results list, detail split panel, pagination, sort.
7. **Create issue**: modal step 1 (project + type), then full form with rich text editor, priority, labels multi-select, attachment dropzone, linked issues, assignee, epic link, sprint.
8. **Dashboard**: gadget grid (Assigned to me table, Activity Stream, etc.), settings/share/export buttons.
9. **Releases**: versions list with progress bars, empty state.