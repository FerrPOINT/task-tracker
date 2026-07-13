# Glossary — Task Tracker

## 1. Overview

Словарь терминов, используемых в документации и коде. Согласованная терминология уменьшает путаницу между backend, frontend, QA и product.

## 2. Terms

| Term | Russian | Definition |
|------|---------|------------|
| Issue | Задача | Основная единица работы: bug, task, story, epic. |
| Project | Проект | Контейнер задач с уникальным ключом (PROJ). |
| Issue Type | Тип задачи | Bug, Task, Story, Epic и т.д. Настраивается per project. |
| Status | Статус | Open, In Progress, Done. Состояние задачи в workflow. |
| Workflow | Workflow | Набор статусов и разрешённых переходов. |
| Transition | Переход | Движение задачи из одного статуса в другой. |
| Kanban Board | Канбан-доска | Визуализация задач по колонкам статусов. |
| Sprint | Спринт | Фиксированный временной интервал Scrum. |
| Backlog | Бэклог | Несортированный список задач проекта. |
| Epic | Эпик | Крупная задача, объединяющая несколько story/task. |
| Version | Версия / релиз | Назначенный релиз для набора задач. |
| Component | Компонент | Логическая часть проекта (frontend, backend). |
| Custom Field | Кастомное поле | Дополнительное поле задачи, настраиваемое админом. |
| JQL | Jira Query Language | Язык поиска задач. |
| Dashboard | Дашборд | Набор gadgets с метриками и списками. |
| Gadget | Гаджет | Виджет на dashboard (chart, filter result, stats). |
| Worklog | Ворклог | Запись потраченного времени по задаче. |
| Saved Filter | Сохранённый фильтр | Именованный JQL-запрос. |
| Notification Scheme | Схема уведомлений | Правила, кто получает уведомления по событию. |
| Permission Scheme | Схема прав | Набор permission для ролей в проекте. |
| Role | Роль | Admin, Member, Viewer и т.д. |
| Group | Группа | Набор пользователей для системных разрешений. |
| Webhook | Вебхук | HTTP callback на внешний URL по событию. |
| Audit Log | Журнал аудита | Запись действий пользователя/системы. |
| Trash | Корзина | Soft-deleted issues до hard delete. |
| Change Log | История изменений | Поля, кто и когда изменил задачу. |
| Schema | Схема | Комбинация issue types, workflow, screens, fields. |

## 3. Naming Conventions in Code

| Concept | Rust module | TypeScript entity |
|---------|-------------|-------------------|
| Issue | `domain::issue` | `Issue` |
| Project | `domain::project` | `Project` |
| Sprint | `domain::sprint` | `Sprint` |
| Workflow | `domain::workflow` | `Workflow` |
| JQL | `jql::parser` | `JqlNode` |
| Attachment | `infra::storage` | `Attachment` |

## References

- `docs/TZ.md`
- `docs/CODE_STYLE.md`
- `docs/ARCHITECTURE.md`
