# Reports — Task Tracker

## 1. Overview

Отчёты доступны на странице `/projects/{key}/reports` для проектов типа `software`.

### 1.1 Common Parameters

| Parameter | Описание |
|-----------|----------|
| `project_id` | UUID проекта |
| `board_id` | UUID доски |
| `sprint_id` | UUID спринта (для sprint reports) |
| `date_from` | Начало периода |
| `date_to` | Конец периода |

## 2. Sprint Reports

### 2.1 Burndown Chart

- Ось X: дни спринта.
- Ось Y: оставшиеся story points / hours / issue count.
- Линии:
  - **Ideal** — линейное снижение от total до 0.
  - **Actual** — реальное снижение по changelog.
- Формула:
  - `remaining = total_story_points - completed_story_points_by_day`.

### 2.2 Sprint Report

| Section | Описание |
|---------|----------|
| Sprint goal | Цель спринта |
| Completed issues | Выполненные задачи |
| Incomplete issues | Невыполненные задачи |
| Removed issues | Удалённые из спринта |
| Added issues | Добавленные после старта |
| Total story points | Сумма по всем задачам |
| Completed points | Выполненные points |
| Velocity | Completed points за спринт |

### 2.3 Velocity Chart

- Бар-чарт по завершённым спринтам.
- Ось X: спринты.
- Ось Y: story points / issue count.
- Группировка: committed vs completed.

## 3. Kanban Reports

### 3.1 Cumulative Flow Diagram (CFD)

- Ось X: дни.
- Ось Y: количество задач.
- Stacked area chart по статусам/колонкам.
- Показывает workload и bottleneck.

### 3.2 Control Chart

- Ось X: дата завершения задачи.
- Ось Y: cycle time (дней от In Progress до Done).
- Среднее, медиана, rolling average.
- Outliers highlighted.

## 4. Issue Reports

### 4.1 Pie Chart — Issues by Status

- Распределение задач по статусам.

### 4.2 Pie Chart — Issues by Assignee

- Распределение задач по исполнителям.

### 4.3 Bar Chart — Issues by Priority

- Count задач по приоритетам.

### 4.4 Created vs Resolved

- Линейный график: создано/решено задач по дням.

## 5. Epic Reports

### 5.1 Epic Burndown

- Ось X: дни.
- Ось Y: оставшиеся story points в эпике.

### 5.2 Epic List

| Column | Описание |
|--------|----------|
| Epic | Название |
| Status | Статус эпика |
| Progress | % завершённых дочерних задач |
| Total issues | Количество дочерних задач |
| Completed | Завершённые |
| Remaining | Оставшиеся |

## 6. Version / Release Reports

### 6.1 Release Burndown

- Прогресс по версии.

### 6.2 Release Status

| Status | Count |
|--------|-------|
| To Do | N |
| In Progress | N |
| Done | N |
| Canceled | N |

## 7. Time Reports

### 7.1 Time Tracking Report

- По пользователям/задачам/периодам.
- Columns: user, issue, date, time spent, description.

### 7.2 Workload Report

- Нераспределённые vs назначенные задачи.
- Workload per assignee.

## 8. Dashboard Gadgets

### 8.1 Built-in Gadgets

| Gadget | Описание |
|--------|----------|
| `assigned_to_me` | Список задач, назначенных текущему пользователю |
| `watched_issues` | Отслеживаемые задачи |
| `filter_results` | Результаты сохранённого фильтра |
| `activity_stream` | Последние события |
| `sprint_burndown` | Burndown активного спринта |
| `sprint_health` | Здоровье спринта |
| `created_vs_resolved` | График создано/решено |
| `pie_chart` | Пирог по статусу/типу/assignee |
| `statistics` | Count задач по фильтру |

### 8.2 Dashboard Layout

- Grid 2/3 columns.
- Drag-and-drop позиционирование.
- Layout сохраняется в `user_dashboard_layout` JSONB.

## 9. Export

### 9.1 Chart Export

- PNG / SVG.
- CSV data export.

### 9.2 Report Export

- PDF (опционально, через headless browser).
- CSV / Excel.

## 10. Implementation Notes

### 10.1 Data Sources

- `issues` + `issue_changelog` для истории статусов.
- `sprints` + `sprint_issues` для sprint-отчётов.
- `worklogs` для time reports.

### 10.2 Caching

- Daily reports кешируются на 1 час.
- Real-time charts пересчитываются при изменении задачи.

### 10.3 Libraries

- `recharts` — графики.
- `@tanstack/react-table` — табличные отчёты.
- `date-fns` — date math.
