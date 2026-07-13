# UX & Product Gaps — Task Tracker

## 1. Overview

Продуктовые и UX-детали, которые часто забывают в технических спецификациях.

## 2. First-Time User Experience

### 2.1 Default Setup

- После первого запуска система создаёт:
  - System admin из env.
  - Demo project `DEMO`.
  - Sample issues: 5 задач, 3 типа, workflow.
  - Welcome page с tour.

### 2.2 Empty States

| Page | Empty State |
|------|-------------|
| Project list | CTA "Create first project" |
| Kanban board | "No issues yet. Create one." |
| Backlog | "Backlog is empty" |
| Reports | "Need more data" |
| Notifications | "No notifications" |

### 2.3 Demo Data Toggle

- `TASKTRACKER_SEED_DEMO=true/false`.
- Demo project помечен флагом `is_demo`.

## 3. Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `c` | Create issue |
| `j` / `k` | Next/previous issue |
| `e` | Edit issue |
| `.` | Command palette |
| `g` + `i` | Go to issues |
| `g` + `b` | Go to board |
| `g` + `p` | Go to projects |
| `Esc` | Close modal |
| `/` | Focus search |

## 4. Command Palette

- `Cmd/Ctrl + K`.
- Поиск по:
  - issues (по key/summary)
  - projects
  - users
  - actions ("Create issue", "Go to settings")

## 5. Undo / Toast Actions

- Delete issue → toast "Issue moved to trash" + "Undo".
- Bulk update → toast "5 issues updated" + "Undo".
- Toast auto-dismiss 5s.

## 6. Accessibility

- WCAG 2.1 AA target.
- Focus trap в modals.
- `aria-label` на иконках.
- Color contrast >= 4.5:1.
- Skip links.
- Reduced motion support.

## 7. Mobile / PWA

- Responsive breakpoints:
  - mobile: < 768px
  - tablet: 768–1024px
  - desktop: > 1024px
- PWA manifest.
- Service worker для offline cache static assets.
- Touch-friendly kanban drag-and-drop.

## 8. Import / Export

### 8.1 Import

- CSV issue import.
- JSON project import.
- Vikunja JSON migration.
- Jira CSV import (field mapping).

### 8.2 Export

- CSV/JSON issues.
- Project archive.
- Audit log CSV.

## 9. Notifications UX

- In-app notification center.
- Email digest daily/weekly.
- Quiet hours per user.
- Per-project notification rules.

## 10. Search & Discovery

- Global search: `Cmd/Ctrl + Shift + K`.
- Recent issues.
- Favorites / pinned projects.
- Search history.

## 11. Time Zone & Locale

- Все timestamps хранятся в UTC.
- Frontend отображает в user timezone.
- Locale форматирует даты/числа.
- First day of week зависит от locale.

## 12. Data Import/Export UX

- Preview before import.
- Progress bar.
- Error report with row numbers.
- Undo import (trash all imported issues).

## References

- `docs/UI_UX.md`
- `docs/USER_STORIES.md`
- `docs/NOTIFICATIONS.md`
- `docs/I18N.md`
