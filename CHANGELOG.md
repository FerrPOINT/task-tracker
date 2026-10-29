# Changelog

## 2026-10-29: Explicit Configured Delivery Start (Draft)

- Problem: configured PM correctly finishes in Backlog / Ready, but there was no
  explicit producer command to start the existing downstream lifecycle.
- Fix: the current owner can start Analysis through the existing lifecycle API
  and `sdlc start` CLI on an enabled project. Migration0111 preserves old rows;
  intent, initial queue and outbox commit together. PM completion alone still
  creates no downstream work. Original-key replay never resets later stages.
- Readback verifies the saved configured start and PM completion instead of
  applying the legacy Backlog-confirmation rule to Draft owner consent. The
  legacy rule remains tested and unchanged. Synthetic role fixtures must also
  retain real local membership; tests do not bypass the artifact access guard.
- Verification: source is published in PR197; targeted PostgreSQL and actual
  HTTP/contract gates are in progress. Generated API/client and final repository
  gates remain open. This package is neither installed nor seven-role live
  acceptance; PM rollout remains independent.

## 2026-10-29: Fileless Writer Rollout Boundary

- Native PM writers are retired by the coordinated Tracker/Fleet/Workflow/CLI
  replacement. Drain active legacy work and reconcile unknown operations on the
  old compatible cohort before switching; retained history stays read-only.
- New configured tasks use persistent service credentials with unchanged
  owner/project/fence checks. Started legacy tasks are not auto-converted.
- Document the existing configuration and synthetic populated-history checks in
  [Local Setup](docs/LOCAL_SETUP.md#31-configured-cli-cohort). All-role Hermes
  acceptance remains runtime release evidence, not a CLI source merge gate.
  This documentation change performs no installation or activation.

## 2026-10-27: PM Publish CLI Discovery

- Fresh PM check: FLEETQA-9 independently reads Workflow but incorrectly treats
  optional legacy lifecycle403, expected first-revision404 and human-only
  answer/confirmation flags as publication blockers. New context/lifecycle/
  requirements help separates those meanings without changing API errors or
  authorization. Required context/Workflow/publication failures remain blockers;
  PM identity still cannot confirm for the owner. Source80f62d13 is published,
  independently reviewed; all30 Linux CLI workflow tests pass (including the new
  discovery case) in8.48s on that exact source. Disposable Compose resources are
  cleaned. The retained PM still has the old binary; source verification is not
  installation or fresh PM acceptance.
- Problem: the live PM guessed question/options JSON because nested publish help
  described flags but not the existing strict payload; Tracker correctly rejected
  it. A later empty prerequisites list was rejected, and changing an execution
  UUID caused a separate correct stale-fence rejection.
- Fix: question/revision publish help now includes typed substantive product
  examples, actual-context fence mapping, revision/CAS/idempotency rules and
  nonempty document prerequisites. No placeholder revision, invented checkpoint
  or owner impersonation is permitted. API, schema, assignments and guards are
  unchanged.
- Completion discovery: nested completion submit help now gives the exact closed
  configured JSON, actual terminal receipt/fence mapping, stdin and original-key
  readback. Legacy lease/native completion remains a separate supported shape;
  the help grants no authority and does not change business APIs.
- Checks:29 CLI workflow tests PASS on c1bc6805, including all three typed help examples,
  malformed shapes, stdin forwarding and error/no-retry contracts; Rustfmt1.88
  and diff-check PASS. Base helper v2 removed its own QA containers/volumes;
  shared caches/evidence remain. Removal of five flat disposable host inputs was
  denied by tool policy; they remain, without another deletion method.
- Live: the same PM saved revisions1/2 and one answered question through CLI.
  The user separately confirmed revision2/hash, retained by Tracker in Draft.
  Actual final evaluation exposed a Workflow future-effect dependency, not a
  missing Tracker permission. After that Workflow correction, PM itself submitted
  completion01a17d1c at11:26 UTC and read back Backlog/Ready. Exact CLI replay
  preserves its ID/time/hash; task-scoped counts remain one task, one completion,
  zero Analysis intents and zero delivery assignments. UI reload retains the
  result. This is assisted existing-task acceptance, not full release readiness.
  New CLI help is published source, not installed into the retained PM container.

## 2026-10-26: Configured CLI PM — Source Verified, Tracker Installed

- Problem: owner consent in Draft and ordinary preconfigured PM credentials did
  not match the legacy native-runtime contract. Consent now stays in Draft;
  only terminal Workflow plus PM completion publishes Backlog / Ready. No
  Analysis intent is created. Legacy tasks and their saved history are retained.
- Standard agent scopes and current assignment/owner/revision checks remain.
  Project owners can explicitly add a verified active service account through
  the member API; human directory selectors remain human-only. Central subjects
  are resolved to local IDs rather than assumed equal.
- Actual checks: configured PostgreSQL lifecycle/replay/no-Analysis test PASS;
  app, CLI, metadata, migration and service-member checks PASS on their recorded
  source revisions. Rust OpenAPI was regenerated, client/parity PASS on d5fca554.
  A disposable copy of all111 existing tables preserves original rows under the
  exact forward105 SQL;38 original ledger rows are retained. QA cleanup completed.
- Normal Build371d42ef / Apply installed d5fca554 as image3f084e40; health200.
  The new PM service identity has verified member access to the acceptance
  project through the owner API. No direct business-state SQL writes were used.
- Next: compatible Fleet installation and permanent CLI configuration, then a
  real PM dialogue and human exact-revision confirmation. Live Backlog / Ready
  and full release gates remain unaccepted; source tests are not that acceptance.

## 2026-10-26: Bounded PM Producer Installed With History Preserved

- Normal selected-service Build22889741/source1ee/backended445/SDKed61 produces
  image79fcae57; receipt SHA
  4d1a80475f376cf0ce9ebd80fe2f40f1d5555bdf3b2113a28741ad7e8cfb0a0b.
- Actual disposable populated-copy migration preserves all97 original public
  tables' original column/row projections and32 old ledger entries; target38
  appends six. Copy result SHA
  6b24ba213b8b53ae1f52c0b9b557465b1ff8656e8d606627ef5d214744911bb5.
  This migration-only check executes zero tests and does not prove source ACLs,
  HTTP behavior or background workers. Protected source dump is retained.
- Normal Applyfe9457b9 is COMPLETE. Backend2cb481f4 runs79fcae57 healthy with
  migration104. Old producer container19045 and all captured old PostgreSQL
  pid/backend_start pairs are absent. The three old recovery receipts remain
  unchanged and fourth4321 is absent. Post-Apply proof SHA
  3cf5747a0c43bde5206061880a5ba537fa603f2bce599e4548bf8ddec5f12d3a.
- Fleet witness rollout, the live fourth callback, useful PM requirements and
  real owner-confirmed Backlog/Ready remain open. No recovery POST, fake ACK,
  attempt reset, owner confirmation or Analyst assignment was performed here.

## 2026-10-26: Bounded PM Recovery Qualified At Source Level

- Current main is integrated, retaining identity103, PM103 and forward104 without
  changing applied migration bytes. The optional manual budget comes only from
  the authenticated Fleet callback; default cap3 and original request hashes stay.
- Nine actual scoped tests pass on c8/SDKed61, including the219.39-second real
  PostgreSQL expiry/CAS/chain/replay/history scenario. The original overall FAIL
  retains its test-only Clippy warning. The subsequent7b one-line redundant
  conversion correction changes no production code or assertion.
- Actual fmt and strict release workspace/all-target Clippy pass on7b. This
  gate-only run executes zero tests; the nine earlier passes are not recounted.
- Genuine Rust OpenAPI export is adopted byte-identically. Client generation,
  parity and standalone strict TypeScript pass using the lock's tool versions;
  this is not a full frontend gate. The original spec-drift FAIL is retained.
- Installed Tracker533 is unchanged. New image/rollout, real Fleet callback,
  useful PM and human-confirmed Backlog/Ready remain open. No Analyst, new MCP,
  controller, scheduler, fake ACK or attempt reset is introduced.

## 2026-10-24: Terminal-Idle Checkpoint Permit Source

- Independent source review found the new original-key GET required writer
  authority despite Fleet's read-only scheduler credential. It now uses the
  existing scheduler_read guard; PG coverage asserts read succeeds while the
  same principal cannot issue a permit. Execution of this regression is pending.
- Added a closed Feedback-only branch to the existing PM lease resume API.
  Trusted Fleet readback and current locked custody permit one exact checkpoint;
  the canonical expired lease, task state and owner consent do not change.
- Reused the existing idempotency ledger with immutable expiry and cross-ledger
  collision checks; no new migration or business queue.
- Added5 app tests and1 isolated PG test. Rustfmt/diff pass; Cargo/PG, generated
  API/client, installation and live PM acceptance remain open.

## 2026-10-23: Settled PM Input Recovery Source Candidate

- Added append-only recovery for a consumed PM input whose renewed lease expired
  before continuation. Original messages, inputs, receipts and assignment stay
  unchanged; issuance requires an authenticated Fleet barrier and exact expired CAS.
- Added typed reader/witness, cross-ledger key guards and additive0103. Original
  resume payloads omit the optional extension and preserve their old hash.
- Format/diff checks pass. Cargo/PG/OpenAPI and live PM acceptance are pending;
  this is a Draft source package, not a verified installation.
- Rollout is Tracker then Fleet. The current main native-spaces/auth/SDK package
  is not silently included in the bounded installed7d6 runtime repair.

- Источник можно выбрать после публикации требований PM, пока владелец ещё
  не подтвердил их. При повторном согласовании через PM новая версия сохраняет
  исходный выбранный источник и прежние binding-записи.
- История SDLC отображает событие выбора источника на русском и английском.

- Владелец проекта задаёт проверенный источник SDLC; новая задача сохраняет его
  неизменяемую версию, а точное подтверждение требований фиксирует binding.
  Прежний неподтверждённый Draft можно явно привязать к источнику без новой
  задачи или PM-чата. Привязка сама по себе не даёт права записи в Git.
- Новые source API получают уникальные OpenAPI operation ID: они не подменяют
  операции routing-policy в сгенерированном клиенте.
- Повтор настройки источника сохраняет точный CAS: `null` не приравнивается
  к недопустимой нулевой версии.

- UI pin обновлён до Base #245: явное недоступное состояние каталога и
  сохранение переходов по известным destinations.

- Исправленный ввод задачи отправляется после первого подтверждённого отказа
  валидации. При неизвестном исходе прежние запрос и ключ операции сохраняются;
  автоматических повторов POST нет.

- Платформенные UI/SDK imports и пути переведены на PDLC и `base-modules/ui`; совместимые readers сохраняют прежние credentials/транзакции. Нормативные термины и границы описаны в `docs/PDLC_BASE_INTEGRATION.md`; установка и включение новых writers выполняются отдельно.

- SSO использует `VITE_AUTH_CLIENT_ID` из конфигурации стенда, включая
  `task-tracker-dev` для локальной разработки. Без переопределения сохраняется
  runtime-клиент `task-tracker`; login, callback, refresh и logout используют
  общую конфигурацию.

- Синхронизирован OpenAPI результата аналитика с Rust-генератором: endpoint,
  request и receipt схемы доступны клиентам; drift gate не падает после #130.

## 2026-10-12 — проектная навигация

Общие Base helpers, сохранённые folds и явное восстановление выбора после ошибки привязки.

- Fleet MCP admission v2 обновляет существующую схему новой миграцией 000093.
  Миграция 000091 и прежняя история сохраняются; downgrade с v2 history
  блокируется до reconciliation. Проверка перехода v1→v2 включена в backend CI.

Все значимые изменения проекта документируются здесь.
Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
версионирование — [SemVer](https://semver.org/lang/ru/).

## [Unreleased]

- Добавлены private identity owner check/freeze/release, immutable aliases, guards
  74 источников и read-only memberships projection для Admin. Проверен точный
  source-компонент и отдельный genuine Auth/Tracker private curl; полный
  cohort/Kubernetes и rollout остаются отдельными gates.

- Обновлён общий UI Base; устранено локальное смещение логотипа Tracker в свёрнутом меню. Защита pending-форм и сохранение неопределённого запроса подтверждены тестами и браузерными сценариями.

- Ошибки чтения голосов и наблюдателей показываются отдельно от пустого результата.
  Действия недоступны до подтверждения соответствующего состояния; повтор
  обновляет только чтение и не отправляет голос или подписку.

- Обычные списки ресурсов используют общий ResourceList Base: отступы, разделители и hover/focus едины; продуктовая сетка и действия сохранены.

- Standalone frontend build проверяет `.base-ui-revision`; SDK pin и legacy fallback сохраняются.

- Логотип оболочки использует общий размер Base 40 px; компактный сайдбар равен высоте хедера (60 px).


- Ответ на clarification не восстанавливает подтверждение прежней immutable
  revision даже при новых evidence: PM публикует новую ревизию требований.

- Umbrella-образ копирует сервер в отдельный файл, а не в каталог исходников
  `server`. Сборка проверяет конечный исполняемый файл до публикации образа.

- Потеря Namespace projection больше не делает управляемый проект свободным:
  каталог исключает его, а повторная привязка под блокировкой возвращает
  `namespace_projection_missing`, сохраняя исходную границу ownership.

- PostgreSQL HTTP fixtures проверяют точный fail-closed ответ при отсутствии
  Fleet native reader и неизменность admission/lease state; история по-прежнему
  не разрешает новые machine writes.

- Закреплён общий Base cursor policy: доступные вкладки, кнопки, селекты
  и пункты меню показывают курсор действия без локальных копий CSS.

- Base обновлён для удержания focus в `ConfirmDialog` во время pending.

- Закреплён проверяемый общий Base candidate; standalone-инструкция выбирает
  активный Namespace pin с legacy fallback, как CI и `scripts/build.py`.
- Инструкция coverage отражает фактические пороги и отдельный локальный gate.
- Прямое создание Issue атомарно продвигает постоянный счётчик номеров;
  purge и сохранение меньшего номера не допускают повторную выдачу номера.

- Добавлен ограниченный machine-only reader сквозных проектов для Forge:
  пагинация, стабильные refs и подтверждённая локальная binding projection.
  Sidebar использует общий Base primitive раскрываемой группы проекта.

- Namespace cohort: стабильные refs, локальные binding projections и lifecycle guards; аддитивные migrations, совместимый rollback и отдельные execution v2 gates. Runtime-приёмка ещё не завершена.


- Prepared Analysis сверяет свежую защищённую конфигурацию Fleet с frozen routing
  и повторно проверяет lease/ACL после HTTP. Это read-only preflight, не native
  admission; runtime readiness и dispatch остаются false.

- PM assignment-scoped credentials больше не преобразуются в legacy user claims:
  legacy API запрещён до привязки пользователя; SDLC ограничен одной канонической
  задачей, разрешёнными методами/операциями и текущим назначением. Чтение и lease
  сверяют последний assignment ledger. Rust/PostgreSQL проверки пройдены;
  runtime handoff пока не подключён.

- Добавлен SDLC-каталог проектов для формы создания PM Draft: keyset-пагинация
  по UUID и только минимальные ID/key/name проектов текущего central subject.
  Доступ проверяется заново для каждой страницы; legacy public/admin bypass,
  локальный UUID и email не предоставляют доступ. Каталог не подтверждает
  readiness агента и не разрешает runtime dispatch.

- PM Draft получил machine-only ownership lease: PostgreSQL TTL 30 секунд,
  heartbeat CAS, идемпотентные receipts и readback текущего/исторического состояния.
  Истечение lease не допускает повторного захвата, replay не продлевает TTL.
  Lease не разрешает runtime dispatch: admission и настоящий PM-сценарий остаются
  отдельной интеграцией. CI явно выполняет оба SDLC HTTP/PG набора в разных БД.
- CI проверяет backend и frontend при изменении Namespace SDK pin или общих
  правил запуска проверок. Такие изменения больше не пропускают build/MSRV gates.

- Backup/restore используют явный профиль workspace и owner через Base.
  Restore требует квалифицированное временное назначение через `--qa-only`.
  Restore больше не принимает старый произвольный физический Compose-проект;
  документация не предлагает перезапись действующей БД. POSIX-тесты проверяют
  передачу аргументов, paths с пробелами и отказ при неполном выборе назначения.

- Подтверждения удаления задачи/worklog возвращают клавиатурный фокус на инициатор либо стабильную вкладку после удаления строки. Ошибка очищается при новом подтверждении; pending записи worklog блокирует повторную отправку, Escape и закрытие. Дробные duration и продуктовые права сохранены; Base закреплён для воспроизводимой общей поставки.

- Центральный каталог пользователей читается без создания чужих профилей.
  Локально отключённая запись больше не вызывает `401` всего списка; новые
  ссылки стабильны и материализуются при входе или явном назначении задачи.
  Исторические UUID сохраняются, email не используется для связывания.

- Дашборд не дублирует кнопку создания задачи из общей шапки; мобильная
  иконка создания доступна без открытия навигации. Маршрут создания сохранён.

- Центральные JWT/PAT-профили используют проверенное имя Central Auth вместо
  части email. Переименование сохраняет локальную identity и исторические ссылки;
  неизменённый профиль не блокирует строку на запись. Отключённый локальный профиль
  не обновляется и не активируется при отказе во входе. Ошибка центральной проверки
  не разрешает fallback к историческому локальному паролю.

- Документирована принятая поставка трёх CLI на sdlc1: backup/restore/rollback, live-проверки, Linux/WSL установка и сохранённые ограничения.

- CLI сохраняет прежние текстовые подтверждения удаления и отметки уведомлений в table/compact; JSON успешных пустых ответов остаётся `{"status":"ok"}`.

- Справка CLI скрывает значения token env variables, сохраняя имена переменных.

- CLI принимает UUID и ключ задачи с серверной проверкой доступа; поддерживает статусы/переходы,
  связи, вложения, custom field values, worklogs, корзину и восстановление. Добавлены file/stdin,
  явное снятие исполнителя, пагинация, JSON ошибок/пустых ответов и безопасный download. CI
  проверяет восстановление удалённого ключа в отдельной PostgreSQL БД. Диагностика скрывает исходные
  и обрезанные credentials, включая newline в secret stdin.

- Повторный явный вход разрешён после отменённого перехода Central Auth;
  автоматический guard не отменяет logout. Отмена перехода не показывает
  ложную ошибку доступности Auth, а ошибка сервера остаётся видимой.

### Changed
- Остальные живые SSO-сценарии Admin (первичный выход, переход к Task и
  пользователь с одноразовой ссылкой) используют меню аккаунта; identity
  проверяется внутри меню, а не в закрытой шапке.
- Живая проверка глобального выхода определяет меню аккаунта по доступному
  control, а не списку продуктов; сохраняет проверки отзыва сессий и PAT
  при одновременном перенаправлении соседней вкладки.
- Task Tracker подключён к общему `PlatformHeader`: один переключатель сервисов,
  единые размеры действий, без повторного поиска в шапке. Создание задачи сохраняет
  выбранный проект и доступно в мобильном меню; аккаунт не дублирует email и не
  допускает повторного выбора выхода во время pending.

- Выход через Central Auth больше не очищает локальный UI до навигации на
  центральный logout endpoint.

### Changed
- Desktop/tablet board получил сохраняемый в URL переключатель обычной и
  компактной плотности без уменьшения 40 px действий или потери metadata.
- С доски убрана вводящая в заблуждение панель локальных участников: в Central Auth
  она не управляла доступом. При создании и редактировании задачи исполнители
  выбираются из активного центрального каталога, с явной ошибкой и повтором загрузки.

### Added
- Live visual QA по локализованному флоту (#69), live CI project CRUD + Workflow touch shell (#66).
### Fixed
- Выбор проекта, вкладки и спринта в отчётах сохраняется в browser history;
  Back/Forward восстанавливают контекст, а default/invalid `tab` получает
  канонический URL без потери соседних параметров.
- Доступные таблицы графиков помещены в clipping-wrapper: длинные ISO-даты
  cumulative flow больше не расширяют документ на mobile и tablet.
- Фильтр и страница уведомлений сохраняются в URL и восстанавливаются после
  reload, Back/Forward и прямого открытия без потери соседних параметров.
- Создание задачи сохраняет выбранный проект в URL, а карточка задачи — активную
  вкладку; прямые ссылки, reload и Back/Forward больше не сбрасывают контекст.
- Вкладка журнала аудита хранится в URL, восстанавливается после прямого
  открытия, reload, Back и Forward, а цели вкладок, формы настройки и загрузки
  аудита имеют минимум 44 px на mobile и 40 px на больших экранах.
- Пагинация backlog хранит `offset` в URL, сохраняет остальные query-параметры
  и восстанавливает страницу после reload, Back и Forward; локальные действия и
  ссылки задач получили устойчивые цели не меньше 40 px.
- Страница корзины проекта сохраняется в URL, восстанавливается после reload и
  browser history, пустой offset вне диапазона возвращается на первую страницу,
  tablet-раскладка больше не сжимает summary до нулевой ширины, а destructive
  actions сохраняют AA-контраст.
- Issue detail actions touch-sized, live workflows верифицированы (#67); live Wiki space navigation (#65); патч аудита dev-зависимостей frontend (#68).
### Added
- Live E2E-покрытие: Workflow- и CI/CD-страницы по темам и viewport (#58, #59); SSO-тест синхронизирован с Admin UI (#60).
- Root AGENTS.md и SECURITY.md (стандарты Base); из индекса удалён артефакт аудита review-findings.md.
### Fixed
- Usability борда и уведомлений (#61).
### Added
- OIDC SSO (SYSTEM_ADMIN 4.2): single provider, authorization code + PKCE S256, single-use state/nonce, JIT-provisioning, identity linking (миграция 0032: `oidc_identities`, `oidc_state`).
- Password reset по email (SYSTEM_ADMIN §1.2): одноразовые SHA-256-hashed токены (TTL 30 мин), no-enumeration 202, атомарное потребление + отзыв refresh-сессий; `POST /auth/password/{request,reset}`; миграция m31 `password_reset_token`; env `TASKTRACKER_RESET_BASE_URL`.
- TOTP MFA (SECURITY.md §2): зашифрованные AES-256-GCM RFC 6238 секреты, replay protection, 8 одноразовых recovery-кодов; challenge при логине (`totp_code`), self-service энроллмент `POST /api/v1/auth/totp/{setup,enable,disable}`.

## [0.2.0] — 2026-08-26

### Added
- Watchers и votes для задач (API + UI + CLI).
- Custom fields: текст/число/дата/select/checkbox, валидация значений по типу поля.
- Components и versions в проектах (API + UI).
- Soft-delete: корзина, restore, purge с каскадным удалением comments/worklogs/attachments/links.
- Notifications: генерация из событий задач, SSE push, настройки email digest.
- CLI: 12 групп команд (auth, projects, issues, sprints, board, comments, labels, search, notifications, reports, admin, members).
- Миграция FK-индексов для 15 внешних ключей.
- 30 backend integration tests (316 total), 28 frontend component tests (88 total).

### Security
- IDOR-фиксы: delete/restore/purge задач, add/remove участника проекта, labels, attachments, issue links, custom fields теперь требуют UserClaims и проверяют права.
- Ошибки Database/Internal больше не раскрывают внутренние детали клиенту (логируются через tracing).
- Board move_issue: валидация принадлежности задачи проекту.

### Fixed
- 21 баг существующего функционала: SSE query keys, search status filter, upload progress, notification mark-as-read, audit log pagination и др.
- CLI: имена полей issue create, URL и метод notification endpoints.
- Issue create возвращает 201 Created.
- Restore неудалённой задачи возвращает 409 Conflict.

### Changed
- Backend декомпозиция: services.rs (2492 строки) → 20 доменных модулей, context.rs (843 строки) → 4 модуля.
- Frontend: единые LoadingState/EmptyState/ErrorState/ConfirmDialog компоненты.
- Native `window.confirm` → shared ConfirmDialog.
- Полный i18n: 393+ ключа RU/EN, захардкоженные строки устранены.
- Accessibility: aria-label на всех icon-кнопках.

### Removed
- Saved filters (API без frontend UI; JQL сохранён — используется search и CLI).
- 20 stale документов, seed demo data, CHANGELOG-заглушка.
- Неиспользуемые зависимости: garde, tokio-test, redis, sqlx (direct), sea-orm из app.

### Docs
- MIGRATIONS.md переписан с Refinery на SeaORM Migrator.
- SECURITY.md: нереализованные controls помечены «не реализовано».
- .env.example и docker-compose.yml: формат env vars `TASKTRACKER_SECTION__KEY`.
- ROUTING.md приведён к 14 реальным маршрутам, DATA_MODEL.md — к фактической схеме.
- API.md: WebSocket секция заменена на SSE.

## [0.1.0] — 2026-08-24

### Added
- Initial release: проекты, kanban board, backlog, sprints, задачи с workflow, labels, comments, attachments, issue links, search (JQL), reports (velocity/burndown/cumulative flow/control chart), auth (JWT + refresh rotation), admin panel, audit log, worklog/time tracking.
- React 19 + Vite 6 + Tailwind 4 frontend, Rust/Axum/SeaORM backend, PostgreSQL.
- Docker Compose: postgres, redis, backend (3456), frontend (19877).
- CI: fmt + clippy + tests + typecheck + lint + build + E2E smoke.
