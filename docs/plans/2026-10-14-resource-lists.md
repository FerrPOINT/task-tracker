# Общие списки ресурсов Base

Утверждённый объём: одинаковые отступы, разделители, hover/focus обычных списков; продукт сохраняет данные, раскладку, маршруты и действия. Backend/skills pins не меняются.

## Контракт

Боковой отступ — 16 px. Normal: вертикальный 8 px, минимум 48 px; comfortable: 16 px, минимум 56 px. Высота не фиксируется. Семантика нативная; контейнер с несколькими действиями не становится кнопкой. CSS Base используется React и штатным SSR-экспортом.

## Покрытие

| Участок | Результат | Основание |
|---|---|---|
| `frontend/src/features/issue-detail/ui/AttachmentPanel.tsx:135` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/features/sdlc/ui/SdlcPanel.tsx:414` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/admin/index.tsx:170` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/admin/index.tsx:285` | Исключено | Таблица с колонками и общим заголовком |
| `frontend/src/pages/dashboard/index.tsx:63` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/dashboard/index.tsx:108` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/notifications/index.tsx:311` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/project-custom-fields/index.tsx:165` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/projects/index.tsx:152` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/pages/search/index.tsx:438` | Перенесено | Обычный список ресурсов или активности |
| `frontend/src/features/issue-detail/ui/TaskDocumentsPanel.tsx:43` | Перенесено | Связанные ресурсы или список сессий |
| `frontend/src/features/issue-detail/ui/TaskRepositoriesPanel.tsx:112` | Перенесено | Связанные ресурсы или список сессий |
| `frontend/src/features/issue-detail/ui/LinkEditor.tsx:43` | Перенесено | Связанные ресурсы или список сессий |

Сайдбары, document trees, маркированные пояснения/ошибки, формы, редакторы, таблицы и схемы исключены по назначению, а не по совпадению CSS-класса.

## Приёмка

Компонентные тесты и Chromium contract: семантика/действия, 16 px, grid, темы, длинный текст, 375/1281/1920 px, native disclosure. Затем продуктовые тесты, lint, typecheck/build, packed SDK и SSR drift. Установленная PDLC1 проверяется через IAB; происхождение images фиксируется отдельно от source/CI. Изменённые сервисы обновляются через private bundle с rollback images.
