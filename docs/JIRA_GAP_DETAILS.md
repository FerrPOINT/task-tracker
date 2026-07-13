# Jira Details Gap Analysis — Task Tracker

## 1. Overview

Детали из Jira, обнаруженные в структурных сэмплах, которые нужно явно учесть в Task Tracker.

## 2. Issue Data Model Gaps

### 2.1 Status Category

Jira каждый `status` привязывает к категории:

- `new` / `todo` (blue-gray)
- `indeterminate` / `in_progress` (yellow)
- `done` (green)

Это влияет на:
- цвета в UI
- board column defaults
- reporting (open/closed)
- workflow validation

Нужно добавить в `statuses`:

```sql
ALTER TABLE statuses ADD COLUMN category TEXT NOT NULL
    CHECK (category IN ('todo', 'in_progress', 'done'));
```

### 2.2 Issue Type Icons

Jira issue types содержат `iconUrl` и `avatarId`. Нужно добавить:

```sql
ALTER TABLE issue_types ADD COLUMN icon_url TEXT;
ALTER TABLE issue_types ADD COLUMN color TEXT;
```

### 2.3 Priority Icons

Jira priorities содержат `iconUrl`. Добавить:

```sql
ALTER TABLE priorities ADD COLUMN icon_url TEXT;
ALTER TABLE priorities ADD COLUMN color TEXT;
```

### 2.4 Status Icons

Jira statuses содержат `iconUrl`. Добавить:

```sql
ALTER TABLE statuses ADD COLUMN icon_url TEXT;
```

### 2.5 Affected Versions

В addition to `fixVersions`, Jira хранит `versions` (affects versions):

```sql
CREATE TABLE issue_affected_versions (
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    version_id UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    PRIMARY KEY (issue_id, version_id)
);
```

### 2.6 Issue Link Types with Direction

Jira ссылки типизированы и имеют направление:

```sql
CREATE TABLE issue_link_types (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    inward_name TEXT NOT NULL,  -- "is blocked by"
    outward_name TEXT NOT NULL, -- "blocks"
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE issue_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    link_type_id UUID NOT NULL REFERENCES issue_link_types(id),
    inward_issue_id UUID NOT NULL REFERENCES issues(id),
    outward_issue_id UUID NOT NULL REFERENCES issues(id),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(inward_issue_id, outward_issue_id, link_type_id)
);
```

Default link types:
- Blocks / is blocked by
- Duplicates / is duplicated by
- Relates to / relates to

### 2.7 Issue Security Level

Jira позволяет ограничить видимость issue внутри проекта:

```sql
CREATE TABLE issue_security_schemes (...);
CREATE TABLE issue_security_levels (...);
CREATE TABLE issue_security_scheme_levels (...);

ALTER TABLE issues ADD COLUMN security_level_id UUID REFERENCES issue_security_levels(id);
```

Scope: future / enterprise. MVP может обойтись без этого.

### 2.8 Description Format — ADF

Jira использует Atlassian Document Format (ADF) для description и comments:

```json
{
  "version": 1,
  "type": "doc",
  "content": [
    {
      "type": "paragraph",
      "content": [
        {"type": "text", "text": "Hello "},
        {"type": "text", "text": "world", "marks": [{"type": "strong"}]}
      ]
    }
  ]
}
```

Решение для Task Tracker:
- Хранить description/comment body как JSONB.
- Внутренний формат близкий к ADFv1 для совместимости.
- Tiptap использует ProseMirror JSON; конвертер ADF <-> ProseMirror.
- Рендеринг через Tiptap / plain text fallback.

### 2.9 Expandable Issue Fields

API должен поддерживать `?expand=changelog,renderedFields,operations,editmeta`:

| Expand | Meaning |
|--------|---------|
| `renderedFields` | HTML/ADF rendered description |
| `operations` | allowed transitions |
| `editmeta` | editable fields metadata |
| `changelog` | history of changes |
| `versionedRepresentations` | versioned content |

### 2.10 Self Links / HATEOAS

Jira возвращает `self` URL на каждый ресурс.

Решение:
- Добавить `self` в JSON response для всех ресурсов.
- Базовый URL из `TASKTRACKER_PUBLIC_URL`.
- Не критично для MVP, но нужно для API consistency.

## 3. Project Gaps

### 3.1 Project Category

```sql
CREATE TABLE project_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT
);

ALTER TABLE projects ADD COLUMN category_id UUID REFERENCES project_categories(id);
```

### 3.2 Project Lead

Jira проект имеет `lead` пользователя:

```sql
ALTER TABLE projects ADD COLUMN lead_id UUID REFERENCES users(id);
```

### 3.3 Component Lead / Default Assignee

```sql
ALTER TABLE components ADD COLUMN lead_id UUID REFERENCES users(id);
ALTER TABLE components ADD COLUMN default_assignee_id UUID REFERENCES users(id);
```

## 4. Board / Kanban Gaps

### 4.1 Column to Multiple Statuses Mapping

Board column может включать несколько статусов:

```sql
CREATE TABLE board_column_statuses (
    board_column_id UUID NOT NULL REFERENCES board_columns(id) ON DELETE CASCADE,
    status_id UUID NOT NULL REFERENCES statuses(id) ON DELETE CASCADE,
    PRIMARY KEY (board_column_id, status_id)
);
```

### 4.2 Estimation Field

Board ссылается на estimation field:

```sql
ALTER TABLE boards ADD COLUMN estimation_field_id UUID REFERENCES custom_fields(id);
-- or built-in: 'story_points'
```

### 4.3 Swimlane Strategy

```sql
ALTER TYPE board_swimlane AS ENUM ('none', 'assignee', 'epic', 'story');
ALTER TABLE boards ADD COLUMN swimlane_strategy board_swimlane DEFAULT 'none';
```

### 4.4 Quick Filters

```sql
CREATE TABLE board_quick_filters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    board_id UUID NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    jql TEXT NOT NULL,
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

## 5. Custom Field Gaps

### 5.1 Schema Types

Custom fields должны иметь schema:

```json
{
  "type": "string|number|date|datetime|user|array|option|any",
  "items": "string|option|user",
  "custom": "pluginKey",
  "customId": 10000
}
```

### 5.2 Clause Names for JQL

```sql
ALTER TABLE custom_fields ADD COLUMN clause_names TEXT[] DEFAULT '{}';
```

JQL поддерживает:
- `cf[10000]`
- `"Epic Link"`
- `"Story Points"`

### 5.3 Epic Link Custom Field

Epic Link — это технически custom field с типом `gh-epic-link`.

Решение:
- Хранить `epic_id` в `issues` как нативный FK.
- Для JQL и кастомных скринов — виртуальный custom field `Epic Link`.

## 6. Time Tracking Gaps

Jira структурирует timetracking:

```json
{
  "originalEstimate": "1w 2d 3h",
  "remainingEstimate": "2d 5h",
  "timeSpent": "3d 2h"
}
```

Наши поля уже есть:
- `original_estimate_seconds`
- `remaining_estimate_seconds`
- `time_spent_seconds`

API должен возвращать человекочитаемый формат + seconds.

## 7. Watchers / Votes

Уже покрыты в DATA_MODEL, но API endpointы должны быть:

- `POST /issues/{id}/watchers` / `DELETE`
- `POST /issues/{id}/votes` / `DELETE`
- `GET /issues/{id}/watchers` — для admin/owner

## 8. Recommendations

| Feature | Priority | Action |
|---------|----------|--------|
| Status category | High | Update `statuses` table |
| Issue type/priority/status icons | Medium | Add icon_url/color columns |
| Affected versions | Low | Add table |
| Issue link direction/types | Medium | Update `issue_links` model |
| ADF description | Medium | Add converter spec |
| Issue expand | Medium | Add API query param |
| Project category | Low | Add table |
| Project/component lead | Medium | Add lead columns |
| Column-status mapping | High | Add junction table |
| Board estimation field | Medium | Add column |
| Swimlane strategy | Medium | Add enum |
| Quick filters | Medium | Add table |
| Custom field schema | High | Enhance custom_fields |
| Custom field JQL clause names | Medium | Add clause_names |
| Self links | Low | Add to API response |

## References

- `docs/DATA_MODEL.md`
- `docs/API.md`
- `docs/UI_UX.md`
- `docs/WORKFLOW.md`
- `docs/JQL.md`
- `docs/JIRA_UI_CAPTURE.md`
- `docs/VIKUNJA_GAP_ANALYSIS.md`
