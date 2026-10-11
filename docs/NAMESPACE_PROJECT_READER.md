# Tracker project presentation reader

`GET /api/v1/namespace-projects?limit=100&offset=0` находится в machine reader
router. Возвращает существующий `ResourceContextSummary[]`: подтверждённый
NamespaceRef, Tracker ResourceRef, generation/lifecycle, имя и ключ проекта.
Строгие SDLC envelopes не расширяются. Limit ограничен 1–100; offset неотрицательный.

Reader требует отдельного субъекта из `TT_NAMESPACE__READER_SUBJECTS` и scope
`task-tracker:read`. Human JWT/PAT, write-only и foreign machine principal
не допускаются. Machine authentication не создаёт human profile и не открывает
`/projects` или остальные human routes. Revoked credential возвращает 401.

Продукт-потребитель читает каталог в фоне и сохраняет проекцию. Имя и ключ
не являются identity или основанием новой привязки. Stored command проверяется
против resource/registry/namespace columns; повреждение возвращает unavailable,
не подменяется соседним проектом. Архивные проекты остаются в каталоге для истории.
