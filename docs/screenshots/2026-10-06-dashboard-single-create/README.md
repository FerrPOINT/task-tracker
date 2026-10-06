# Dashboard: Одна Команда Создания

Проверено 6 октября 2026 года через настоящий OIDC и API временного
Compose-проекта с отдельной БД. API не подменялись, рабочие данные не читались.
Использован production bundle после frozen install, codegen, lint, typecheck,
264 frontend tests и build; он подключён к nginx read-only bind mount.

Матрица: light/gray/dark, 320, 375, 768, 1280, 1920 и 2560 px. Все 18 состояний
имеют одну видимую команду создания, target не меньше 40 px, без горизонтального
переполнения, неожиданных scroller, console/network errors и serious/critical axe.
Переход с клавиатуры проверен 18 раз, касанием на 320/375/768 - 9 раз.

Представительные full-page screenshots:

- `light-375.png`: 375 x 812, mobile.
- `dark-1920.png`: 1920 x 1080, desktop.
- `light-2560.png`: 2560 x 1440, wide desktop.

Runtime приложения не заменён. Свой временный проект удалён; permanent
container IDs, images и started-at не изменились. Этот результат проверяет
скомпилированный UI, но не подменяет проверку окончательного Docker-образа
или общий final-main platform gate. Остальные backend images унаследованы
из предыдущего квалифицированного QA cohort, Workflow не final master.
QA secrets и private receipts в репозиторий не включены.
