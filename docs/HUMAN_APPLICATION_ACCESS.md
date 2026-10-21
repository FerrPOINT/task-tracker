# Две роли и приложения: интеграция Tracker

Статус: исходники feature-ветки; до dev-поставки и активации.
Канон: Base `docs/platform/HUMAN_APPLICATION_ACCESS.md`, ADR0011.

Central Auth владеет человеческой ролью и приложением `task-tracker`.
Локальный профиль сопоставляется по неизменному central subject; JIT не выдаёт
systemadmin и не переписывает исторические назначения. Ответ собственного
профиля показывает текущую роль, сохранённые флаги не являются authority.

Обычные задачи, комментарии, вложения и значения полей остаются рабочими
операциями. `Authz` отделяет их от настройки структуры: проекты/участники,
справочники, определения полей, labels/components/versions, постоянное удаление
и административное исправление чужих записей требуют текущего central admin.
Само наличие приложения либо собственность проекта не выдаёт эту роль.

Bearer middleware связывает проверенную центральную authority с локальным UUID
в request SDK scope. SDLC имеет собственный strict middleware: human scope
связан с центральным UUID, PM/delivery/machine principals не получают human role.
Human confirmation и работа исполнителя не превращены в admin-only операции.
Управление source/routing policy и привязками репозиториев требует admin;
технические scopes/fences и строгие версии execution contexts сохраняются.

Namespace admission независим и обязателен при интеграции; текущий feature
checkout не заменяет незавершённые ветки пространств. Рабочие роли SDLC и
профессии Workflow не являются человеческими ролями платформы.

Открытые проверки до активации: адресные SQL/API tests, транзакционные proof
expiry guards, приложения всей Tracker/Fleet/Workflow/Forge цепочки,
скачивания и фоновые стадии, клиентские controls/cache, filled migration,
совместимый cohort, dev/IAB и реальное задание. Scoped source-check не означает
выполненную поставку или проверку этих границ.
