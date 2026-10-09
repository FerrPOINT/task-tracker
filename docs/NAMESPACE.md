# Namespace, каталог проектов и контекст задачи

Admin владеет Namespace; Tracker владеет Project, Tasks, командой и SDLC
assignments. Local binding разрешает NamespaceRef в реальный Project UUID;
имя и key не служат идентичностью. Ready binding читается локально при Admin
outage; новые bindings требуют Admin. Потерянная projection закрывает writes.

## API и данные

Machine owner API: `GET/PUT /api/v1/namespace-resources/tracker_project/{id}`.
Human contexts/catalog/stats: `/api/v1/namespace-contexts`,
`/namespace-available-resources`, `/namespace-stats/{registry}/{namespace}`.
Human Project directory использует shared-trusted policy и bounded pagination;
пакетные counters не делают отдельный запрос на каждый Project.
Machine scopes, creator receipts и exact-owner confirmations сохраняются.
Проверенная human session может создать Draft в общем Project. Readback
остаётся author-scoped: чужой ключ операции возвращает 404, approvals требуют
точного owner, PAT без human session и machine credentials не создают Draft.
Удаление записи команды не отзывает эти права человека.
SDLC v1 service/PAT readers сохраняют явный project scope и assignment fences;
проверка общего human/PAT каталога не выдаёт им human session.

Migration 0090 добавляет confirmed bindings, immutable managed marker,
`project_issue_counters`, creation receipts и `task_repository_links`.
Обычные Issues и Draft foundation 0034 используют один atomic allocator;
original-key replay проверяется до выдачи номера. После конфликта со старым
прямым INSERT counter продвигается по подтверждённой записи, без MAX+1.
Выданные новым counter номера
не переиспользуются после purge. Начальный high-water mark включает имеющиеся
данные; неизвестные ранее удалённые номера восстановленными не объявляются.
Project keys сохраняют прежний валидатор: 1–10 ASCII букв, цифр или дефисов.

Task UUID используется в межпродуктовых links. Human `/api/v1/tasks/{id}`
subresources `documents`, `repositories`, `available-repositories` и
`delivery-evidence` показывают immutable revisions, impacted repositories,
PR/commits и точные CI checks. Wiki владеет document link; Tracker читает его
ограниченным reader. Сохранённые repository links читаются без Forge callback.
Недоступность владельца показывается unavailable, а не пустым каталогом.

Archive закрывает mutations всех старых и новых paths и физический delete
managed Project. Restore сохраняет IDs. Foundation reservation без verified
release не считается drained по истёкшему lease или `dispatch_allowed=false`;
Archive остаётся pending с исходной operation до owner reconciliation.

## Настройка и поставка

`TT_NAMESPACE__INSTANCE_ID`, `REGISTRY_INSTANCE_ID`, `OWNER_SUBJECTS`,
`READER_SUBJECTS` задаёт deployment. Readers: `TT_NAMESPACE__WIKI_URL`,
`WIKI_TOKEN_FILE`, `FORGE_URL`, `FORGE_INSTANCE_ID`, `FORGE_TOKEN_FILE`.
Они не получают forwarded human PAT. Machine principal определяется раньше
human mapping; personal/owner-only ограничения SDLC не становятся общим ACL.

UI включается `VITE_NAMESPACE_ENABLED=true` после совместимого cohort.
NamespaceRef находится в URL и query keys; ошибочный binding не выбирает
первый Project. Старые SDK/skills pins и строгие SDLC v1 envelopes сохраняются.

Верхний селект и боковая навигация читают один каталог confirmed bindings,
включая следующие страницы. «Все проекты» показывает блок каждого проекта;
новые группы раскрыты по умолчанию, блоки сворачиваются независимо. Состояние сохраняется в localStorage
по стабильному ResourceRef и восстанавливается после перезагрузки или возвращения
в Tracker, включая ранее развёрнутые группы. Выбранный Namespace показывает только
свой блок, сохраняя его запомненное состояние. SSO callback восстанавливает
точный return URL, не меняя фильтр каталога. URL-параметр
`project_scope=all` сохраняет фильтр «Все проекты» отдельно от NamespaceRef
открытого ресурса. Переходы внутри Tracker не сужают этот фильтр; прямой
Namespace URL без параметра сохраняет прежний выбор одного проекта. Ссылки доски,
бэклога, отчётов, корзины и настроек передают Namespace своего блока. Desktop
и mobile drawer используют одну навигацию; длинный список имеет свой scroll.

Для прямых owner-запросов wizard/counters задаётся явный список origins
через `TASKTRACKER_SERVER__CORS_ALLOWED_ORIGINS` (значения через запятую).

Owner/readers и SDLC проверяют identity/scopes независимо от display metadata.
PAT старого Auth без имени использует только существующий active профиль с
точным central subject; новый профиль и повышение роли из отсутствующих
metadata не создаются. Machine subjects отсекаются до human profile mapping.
