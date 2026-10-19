# Каноническая M:N-связь проекта с пространствами

Статус: source-кандидат; миграция и новый доступ в dev не активированы.
Общий договор — Base ADR 0010 и `docs/contracts/UNIFIED_SPACES_V1.md`.

## Изменение и проверка полномочий

`POST /api/v2/projects` принимает непустой уникальный `spaces: SpaceRef[]`.
Проект, доска, первые связи и durable projection intent сохраняются одной
транзакцией. Обычное создание через старый API закрывается после активации.

`GET /api/v2/projects/{project_key}/spaces` возвращает реальный текущий набор
и его ограниченное исходным сроком подтверждение. Этот ответ не является
постоянной командой проекции или самостоятельным bearer-полномочием.

`PUT /api/v2/projects/{project_key}/spaces` принимает `operation_id`,
`expected_revision`, `spaces`. Нужны подтверждённая центральная identity,
прежние полномочия управления проектом и разрешённое активное пространство
из текущих связей. Только добавляемые связи требуют нового активного допуска.
Сохранённые архивные или скрытые связи не удаляются и не вставляются повторно.
Последняя связь не снимается; commit constraint также действует при прямом SQL
после активации. Архивирование последнего пространства сохраняет проект.

Одновременные изменения сериализуются на проекте. Ревизия проверяется внутри
транзакции; ожидание блокировки не продлевает полномочие. Новый ключ со старой
ревизией получает конфликт. Тот же исходный ключ с другим проектом, identity
или payload отклоняется. Повтор того же замысла возвращает фактический текущий
набор без повторной записи; исторический результат не выдаётся как свежее
подтверждение старых связей. Смена названия не меняет идентичность.

## Проекция Wiki

Canonical commit атомарно сохраняет очередь `tracker_project_space_outbox`.
Команда `ProjectSpacesProjection` имеет отдельную строгую схему: schema_version1,
operation_id, ProjectRef, revision, непустой spaces и optional legacy_project.
Она не содержит TTL или пользовательских прав. Импортируемый NamespaceRef
берётся из прежнего ID binding Tracker; по имени или первому проекту не выбирается.

`GET /api/v2/projects/{project_key}/spaces/projection` показывает подтверждение
текущей ревизии. Отсутствие intent для актуального набора — ошибка, а не успех.
`POST .../spaces/projection/reconcile` доступен владельцу проекта с действующим
допуском чтения и product scope записи. Он передаёт сохранённые команды по
возрастанию ревизий, максимум восемь за запрос. Pending возвращает HTTP202;
только точные ACK всех предшествующих команд и текущей ревизии дают HTTP200.
Истечение исходного допуска не становится подтверждением доставки.

Оператор задаёт `TT_SPACES__WIKI_ORIGIN` (только origin) и
`TT_SPACES__WIKI_TOKEN_FILE` (защищённый файл отдельного Tracker PAT).
Redirects запрещены; единый HTTP client, timeout5s, ответ до1MiB. Токен пользователя
не пересылается. Ошибка или потеря ответа сохраняет pending и безопасный код;
повтор использует исходную команду. Durable receipt Wiki защищает от дублей.
Принятая доставка не означает завершение миграции/активации общего cohort.

## Чтение серверными потребителями

`POST /api/v2/project-contexts/query` — отдельный registered-reader endpoint с
exact task-tracker:read PAT и allowlist `TT_SPACES__CONTEXT_READER_SUBJECTS`.
Исходную identity и общий deadline берут из проверенного контекста потребителя;
это не новая user-role, PAT delegation или job capability. Ordinary endpoints
не принимают actor metadata от клиента. Сервисная identity не проходит human JIT.

Request явно содержит tracker_instance_id, policy_revision и selector `projects`
(ProjectRef[]) либо `space` (SpaceRef/limit/offset). По Space сначала проверяется
допуск именно к нему. Canonical EXISTS фильтрует проекты до COUNT/LIMIT/OFFSET;
M:N не создаёт дублей. Count и страница читаются из одного SQL snapshot. Read
through A возвращает Project A+B, но selector B без допуска к B запрещён.

Полный ProjectSpaceContextBatch не строится из отстающей Wiki projection.
Оригинальные сроки сохраняются, смена policy revision отклоняется; unknown и
denied ID selectors не раскрывают существование. Размеры запроса/ответа и общих
ассоциаций ограничены. Полная cross-service/auth/serving qualification и UI
этого source-кандидата остаются отдельными gates.

## Обычные пользовательские запросы и исторические данные

После активации middleware подтверждает identity и общий допуск один раз для
запроса. Привязка к local UUID проверяется явно; profile JIT не выдаёт права.
`Authz` использует этот контекст для прежних project read/edit/owner ports.
Чтение по общему допуску не создаёт membership либо owner role. Запись требует
также write scope и разрешённое активное пространство самого пользователя.
Метод чтения и недостаточный write scope различаются: второе не становится
обходом проверки записи. Нет frame при активной политике — нет доступа.

Список проектов фильтруется SQL EXISTS по паре registry/space до pagination.
Внутренний полный набор не передаётся старому helper, где limit0 означает одну
строку. Данные page читаются согласованно; M:N не дублирует project IDs.

Аудит `project`/`issue` фильтруется по реальному проекту ресурса. Категории
`user` и `system_setting` сохраняют существующее глобальное правило чтения;
оно не даёт права редактировать пользователя. Неизвестная категория закрыта.
Уведомления о project/issue дополнительно ограничены собственным получателем.
Текст, pagination и unread count используют одинаковый predicate; отозванные
уведомления сохраняются в БД, но не выдаются. Личные настройки уведомлений и
собственное read-state не предоставляют доступ к тексту чужого проекта.

HTTP body проверяет исходный срок и закрывается даже без новых байтов; новый
срок на poll не вычисляется. SSE явно захватывает тот же request proof для
проверки каждого проектного события. Protected ответы private/no-store. Это не
заменяет очистку frontend caches при identity/policy/context switch.

### Фоновая рассылка

При активной политике digest использует существующие admission URL/token и
общий HTTP bridge/client. Дополнительная операторская регистрация
`TT_SPACES__ADMISSION_READER_SUBJECTS` содержит subject отдельного service account
с exact `admin-panel:read` PAT. Новые секреты либо права пользователям автоматически
не выдаются. Отсутствие регистрации/подтверждения не допускает legacy fallback.

Получатель определяется только по активной сохранённой central-subject binding,
не email или вычисленному ID. Auth заново подтверждает known/active identity;
SDK получает её допуск, а не доступ сервисной identity. Непустое старое сообщение
не является разрешением. Страницы уведомлений повторно читаются с видимостью
получателя и ограничены 10000 записями; превышение — ошибка, а не тихая обрезка.
Письмо отправляется только в исходном окне Auth/policy/token, включая SMTP timeout.
Существующий receipt отмечает только фактически переданные notification IDs.
Это read-only processor proof, не browser session, PAT delegation или job grant.

## Открытые gates

Полная ревизия остальных read/write/PAT/job поверхностей, проверка рассылки
на реальном совместимом cohort, выдача и обновление ограниченных execution grants, M:N UI и общий picker,
полная заполненная миграция/сравнение прав, cohort/rollback и dev/IAB приёмка
должны быть завершены до активации. Native SQL/HTTP fixtures не заменяют эту приёмку.


Независимый архив проекта и его отдельная CAS-операция описаны в
[PROJECT_LIFECYCLE_V2.md](PROJECT_LIFECYCLE_V2.md). Space lifecycle его не изменяет.


## Каталог доступных пространств

GET /api/v2/spaces/visible читает каталог только исходного человека с
task-tracker:read через fresh central request context; admin-panel scope
пользовательскому PAT не добавляется. Backend использует общий SpaceCatalogClient,
TT_SPACES__CATALOG_URL (Admin POST /api/v2/spaces/visible/query) и существующий
секрет TT_SPACES__ADMISSION_TOKEN зарегистрированного read service account.
Ни credential, ни исходный bearer не возвращаются во frontend.

Параметры limit1..10 (default10), offset>=0 (default0), q<=200 Unicode символов.
Неизвестные поля/неверные bounds отклоняются400. Ответ — generated
VisibleSpaceCatalogV2: exact central identity, policy_revision, исходная validity,
spaces/total/limit/offset/has_more; hidden spaces не попадают в count/page.
401 — нет central identity; 403 — нет original read proof/истёк срок;
503 — owner verification unavailable. Legacy/operator fallback отсутствует.

Каталог не разрешает запись в проект. M:N management дополнительно проверяет
существующие project/link roles и active admitted spaces. SpaceRef не выбирает
единственный проект. Downstream catalog validity сужает middleware header и
body guard через внутренний typed proof; чтение raw header не расширяет срок.
UI и полная межсервисная/live квалификация остаются отдельными gates.
