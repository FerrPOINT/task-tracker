# Jira UI/UX Complete Capture Report

## Source
- Instance: https://task.wemakedev.ru
- User: Александр Жуков (azhukov)
- Access: read-only. **No data modified.**
- Server: Jira Server 9.17.5

## Captured Data Overview
- Projects: 10
- Issue Types: 12
- Statuses: 23
- Priorities: 5
- Resolutions: 6
- System Fields: 42
- Custom Fields: 16
- Users: 61
- Issue Link Types: 4
- Boards: 8
- Rapidviews: 8
- Sample Issues (full fields): 1015
- Project Details: 10
- Editmeta fields sample: 9
- Transitions sample: [{'id': '11', 'name': 'Backlog', 'description': '', 'opsbarSequence': 2147483647, 'to': {'self': 'https://task.wemakedev.ru/rest/api/2/status/10002', 'description': '', 'iconUrl': 'https://task.wemakedev.ru/', 'name': 'Backlog', 'id': '10002', 'statusCategory': {'self': 'https://task.wemakedev.ru/rest/api/2/statuscategory/2', 'id': 2, 'key': 'new', 'colorName': 'default', 'name': 'К выполнению'}}}, {'id': '21', 'name': 'To Do', 'description': '', 'opsbarSequence': 2147483647, 'to': {'self': 'https://task.wemakedev.ru/rest/api/2/status/10000', 'description': '', 'iconUrl': 'https://task.wemakedev.ru/', 'name': 'Сделать', 'id': '10000', 'statusCategory': {'self': 'https://task.wemakedev.ru/rest/api/2/statuscategory/2', 'id': 2, 'key': 'new', 'colorName': 'default', 'name': 'К выполнению'}}}, {'id': '31', 'name': 'In Progress', 'description': '', 'opsbarSequence': 2147483647, 'to': {'self': 'https://task.wemakedev.ru/rest/api/2/status/3', 'description': 'В данный момент по этой проблеме ведется активная работа назначенным лицом.', 'iconUrl': 'https://task.wemakedev.ru/images/icons/statuses/inprogress.png', 'name': 'В работе', 'id': '3', 'statusCategory': {'self': 'https://task.wemakedev.ru/rest/api/2/statuscategory/4', 'id': 4, 'key': 'indeterminate', 'colorName': 'inprogress', 'name': 'В работе'}}}, {'id': '41', 'name': 'Review', 'description': '', 'opsbarSequence': 2147483647, 'to': {'self': 'https://task.wemakedev.ru/rest/api/2/status/10201', 'description': 'Этот статус управляется Jira Software на внутреннем уровне', 'iconUrl': 'https://task.wemakedev.ru/', 'name': 'Review', 'id': '10201', 'statusCategory': {'self': 'https://task.wemakedev.ru/rest/api/2/statuscategory/4', 'id': 4, 'key': 'indeterminate', 'colorName': 'inprogress', 'name': 'В работе'}}}, {'id': '51', 'name': 'Done', 'description': '', 'opsbarSequence': 2147483647, 'to': {'self': 'https://task.wemakedev.ru/rest/api/2/status/10001', 'description': '', 'iconUrl': 'https://task.wemakedev.ru/', 'name': 'Готово', 'id': '10001', 'statusCategory': {'self': 'https://task.wemakedev.ru/rest/api/2/statuscategory/3', 'id': 3, 'key': 'done', 'colorName': 'success', 'name': 'Выполнено'}}}, {'id': '61', 'name': 'Canceled', 'description': '', 'opsbarSequence': 2147483647, 'to': {'self': 'https://task.wemakedev.ru/rest/api/2/status/10202', 'description': 'Этот статус управляется Jira Software на внутреннем уровне', 'iconUrl': 'https://task.wemakedev.ru/', 'name': 'Canceled', 'id': '10202', 'statusCategory': {'self': 'https://task.wemakedev.ru/rest/api/2/statuscategory/3', 'id': 3, 'key': 'done', 'colorName': 'success', 'name': 'Выполнено'}}}]
- Comments sample: 0
- Watchers sample: 1

## Issue Detail Fields Observed
`issuetype`, `timespent`, `project`, `fixVersions`, `customfield_10111`, `aggregatetimespent`, `resolution`, `customfield_10107`, `customfield_10108`, `customfield_10109`, `resolutiondate`, `workratio`, `lastViewed`, `watches`, `created`, `priority`, `customfield_10100`, `customfield_10101`, `customfield_10102`, `labels`, `customfield_10103`, `timeestimate`, `aggregatetimeoriginalestimate`, `versions`, `issuelinks`, `assignee`, `updated`, `status`, `components`, `timeoriginalestimate`, `description`, `timetracking`, `archiveddate`, `attachment`, `aggregatetimeestimate`, `summary`, `creator`, `subtasks`, `reporter`, `customfield_10000`, `aggregateprogress`, `customfield_10200`, `environment`, `duedate`, `progress`, `comment`, `votes`, `worklog`, `archivedby`

## Screens and UI Flows Covered
1. Login / Auth (token-based)
2. Top navigation + project switcher
3. Project list / directory
4. Project overview + settings
5. Issue detail view (summary, description, fields, comments, attachments, links, subtasks, worklog, activity, watchers, votes)
6. Kanban board (columns, cards, WIP, quick add)
7. Backlog / Sprint planning (rapidview data)
8. Search / JQL (sample queries)
9. Filters (favourite)
10. Dashboard (system)
11. User picker / mentions
12. Create issue (createmeta fields)
13. Edit issue (editmeta)
14. Issue transitions

## Files in docs/assets/jira-reference/
- `admin_schemes.json`
- `board_10_epics.json`
- `board_10_issues.json`
- `board_10_quickfilters.json`
- `board_11_epics.json`
- `board_11_quickfilters.json`
- `board_12_epics.json`
- `board_12_quickfilters.json`
- `board_13_epics.json`
- `board_13_quickfilters.json`
- `board_1_epics.json`
- `board_1_quickfilters.json`
- `board_2_epics.json`
- `board_2_quickfilters.json`
- `board_3_epics.json`
- `board_3_quickfilters.json`
- `board_4_epics.json`
- `board_4_quickfilters.json`
- `comment_PT_21.json`
- `comments_PRES_99.json`
- `comments_PT_21.json`
- `comments_REL_335.json`
- `createmeta.json`
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
- `filters_favourite.json`
- `filters_search.json`
- `groups.json`
- `issuePicker.json`
- `issue_rendered_pres99.json`
- `issues_full_100.json`
- `issues_full_50.json`
- `issues_sample_200.json`
- `issues_sample_50.json`
- `issues_subset_30.json`
- `issues_with_worklogs.json`
- `issuesecurityschemes.json`
- `jira-reference-data.json`
- `jira-ui-ux-summary.json`
- `jira_issue_PT_21.json`
- `jira_issue_REL_335.json`
- `jira_issue_pres99.json`
- `priorities.json`
- `project_AAT_details.json`
- `project_components.json`
- `project_details.json`
- `project_roles.json`
- `project_versions.json`
- `rapidview_backlogs.json`
- `rapidviews.json`
- `serverInfo.json`
- `sprint_1_issues.json`
- `sprints_board_1.json`
- `subtasks_sample.json`
- `transitions_PT_21.json`
- `user_azhukov.json`
- `users.json`
- `votes_PT_21.json`
- `watchers_PT_21.json`
- `worklog_PT_21.json`
- `worklogs_PT_21.json`

## HTML Mockups
- `docs/assets/ui-mockups/issue-detail.html`
- `docs/assets/ui-mockups/kanban-board.html`
- `docs/assets/ui-mockups/project-list.html`
