# AGENTS.md — Task Tracker

## Docker и Kubernetes

Поддерживаются обычный Docker/Compose без Kubernetes и Kubernetes. Общие API,
данные, настройки и правила исполнения сохраняют одинаковый смысл в обоих
режимах; особенности оркестратора принадлежат deployment-адаптерам.
Перед изменением runtime/config/storage/runner читать [стандарт Base](https://github.com/FerrPOINT/services-base/blob/29a709834fba54a823873b82050731333d7e3d97/docs/REPO_STANDARDS.md#поддержка-docker-и-kubernetes).
Затронутый сценарий проверять отдельно в обоих режимах на точном кандидате;
непроверенный режим оставлять открытым gate. Docs-only правки не требуют deploy
или пересборки. Ссылка фиксирует решение Base, не меняя SDK/UI/skills pins.

## Репозиторий

- **GitHub**: `git@github.com:FerrPOINT/task-tracker.git`
- **Стек**: backend Rust (Axum + SeaORM + PostgreSQL), frontend React 19.1.0 + Vite 6.2.0 + Tailwind CSS 4.1.0
- **Env prefix**: `TASKTRACKER_`
- **Публичные порты по умолчанию**: frontend docker `19877`, backend `3456`. PostgreSQL и Redis внутренние (compose-сеть), наружу не публикуются

## Правила работы

### 1. Перед началом работы

1. Прочитать `docs/TZ.md`, `docs/ARCHITECTURE.md`, `docs/DATA_MODEL.md`.
2. Проверить текущее состояние ветки: `git status`.
3. Составить план, показать пользователю, получить подтверждение.

### 2. Код

- Backend: слоистая архитектура `controller → service → repository`.
- DI через `AppContext`.
- Все публичные API покрыты OpenAPI через `utoipa-axum`.
- Rust-хендлеры и DTO — единственный источник правды для схемы; фронт генерирует клиент из `openapi/openapi.json`.
- Все endpoint тестируются интеграционно через testcontainers.
- Frontend: публичные компоненты `@pdlc/ui` + Tailwind; предметные композиции остаются в продукте.
- Состояние: серверное — `@tanstack/react-query`, клиентское — `zustand`.
- Формы — `react-hook-form` + `zod`.

### 3. Коммиты

- Conventional commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`).
- Один коммит = одна логическая единица.
- Не amend/squash без явного запроса.
- Push только после проверки `cargo test`, `pnpm typecheck`, `pnpm test`.

### 4. Тестирование

- Backend: `cargo test`, интеграционные тесты с PostgreSQL testcontainer.
- Frontend: Vitest + Playwright.
- После UI-изменений — скриншоты full-page (375 / 1920 / 2560).
- Все новые endpoint — curl-проверка.

### 5. Документация

- При изменении API обновлять `docs/API.md`.
- При изменении дата-модели обновлять `docs/DATA_MODEL.md`.
- При новом функционале добавлять/обновлять `docs/TZ.md`.
- Любые неочевидные решения фиксировать в `docs/ARCHITECTURE.md`.

### 6. Безопасность

- Никогда не коммитить credentials, токены, пароли, email коллег, реальные данные клиентов.
- Все secrets — через env vars.
- Перед push проверять, что в diff нет чувствительных данных.

### 7. Docker

- Сборка: `docker compose build`.
- Пересоздание контейнера: `docker compose up -d` (не `docker compose restart`).
- Проверка: `docker compose ps` и health endpoint.

### 8. Проверка перед завершением

- [x] Все тесты проходят.
- [x] Линтеры (`clippy`, `eslint`, `prettier`) чистые.
- [x] Документация актуальна.
- [x] Коммиты запушены в `origin/main`.
- [x] Пользователь увидел результат (скриншот / curl / лог).

## Контакты

- Техлид: Александр Жуков.
- Основной язык общения и документов: русский.

## References

- `docs/ARCHITECTURE.md`
- `docs/CODE_STYLE.md`
- `docs/TESTING.md`

## Base и воспроизводимость

См. [BASE_INTEGRATION](docs/BASE_INTEGRATION.md). Обязательны pinned Base SHA,
locked/frozen зависимости и проверка актуальных checkout до сборки.
Rust build toolchain 1.88.0, Node 22.20.0, pnpm 10.28.1; MSRV отдельно.

## Local Docker ownership

Every local container uses real Compose. Permanent projects are sdlc1, sdlc2
and sdlc-common. Temporary sdlc-qa-/sdlc-build- projects require task/purpose,
explicit endpoint and scoped cleanup in finally. Direct docker run/create,
fake Compose labels and global prune are forbidden. Runtime data, snapshots,
keys and shared caches are preserved. Base owns tools and LOCAL_GROUPS rules.

## Термины PDLC и общие компоненты

Перед изменением платформенных имён прочитайте [интеграцию с Base](docs/PDLC_BASE_INTEGRATION.md) и канонические TERMS/BRANDING.
Работа с UI описана в разделе «Разработка UI со справочником Base» ниже.
Не переносите продуктовые запросы, DTO и права в общий UI. SDK/UI/skills pins независимы; новое имя платформы не меняет SDLC IDs/scopes или Hermes upstream.

## Общие поля выбора Base

Сначала искать общий UI в Base: search → show → example. Общие исправления
вносить в Base; продукт передаёт данные, права, преобразования и сохранение.
Для закрытого списка с одним значением — Select; для нескольких — MultiSelect.
Одно значение из всех видимых вариантов — RadioGroup, взаимноисключающие режимы — appearance="segmented".
Вкладки с панелями используют Tabs. Каталог с поиском/пагинацией — SearchableSelect.
Использовать публичный @pdlc/ui и сверять наличие API по точному UI pin.
Контракт Select/MultiSelect: options, value/defaultValue, onValueChange;
HTML children/option/onChange/multiple/size больше не являются их API.
Видимые raw select и локальные копии оформления не добавлять. Технический form
bridge принадлежит только Base; SSR использует штатный macro и choice-runtime,
явный refresh после программных value/options и native fallback до успешного init.
Правила выбора и подключения: Base docs/platform/CHOICE_CONTROLS.md.
UI/SDK/skills pins независимы; установка и production — отдельные операции.

## Разработка UI со справочником Base

Открытая страница справочника может быть новее зависимости продукта: сверять
контракт по принятому UI pin. Если поиск пуст, уточнить запрос/фильтры и
проверить рецепты; отсутствие API проверить отдельно в актуальном Base.
Общий контракт добавлять в Base вместе с metadata и примером, затем явно
обновлять UI pin потребителя. Локальная копия не заменяет эту интеграцию.

Перед созданием или изменением UI читать [маршрут разработки со справочником Base](https://github.com/FerrPOINT/services-base/blob/ae2b41291eee5af752daff89f760fba5aff41783/base-modules/ui/reference/guides/agent.md).
Из корня продукта с подготовленным frontend: `pnpm --dir frontend exec pdlc-ui`.
Сначала сверить `.base-ui-revision` и выполнить `check` один раз в начале задачи
или после смены UI pin; затем `search "задача"` либо адресный `show`, при
необходимости `--section props` или `--section setup`, и один `example`. Использовать точный
квалифицированный адрес из поиска; композиции доступны через `example recipes`.
Без подготовленного frontend руководство описывает CLI из отдельного checkout
того же UI SHA без npm install. Не загружать весь каталог в контекст и не
подменять принятую версию через npx/dlx/latest или скрытый HTTP fallback.
Проверить providers/Router/i18n/CSS и cleanup; после интеграции — scoped проверки
продукта и IAB затронутого поведения. Источники, hosted CI и runtime — разные gates.

Button, Input, Textarea, Checkbox, RadioGroup/Radio, Switch, меню, Card, Table, Disclosure,
Dialog и Toaster/toast использовать из публичного @pdlc/ui. Общий дефект
исправлять в Base; продукт сохраняет данные, права и сохранение. Прямой Sonner,
копии примитивов и продуктовые определения общих tokens не добавлять.
Брендинг применяется общим runtime; draft preview изолируется BrandingScope.
Размеры однострочных контролов и типографика/padding TabsTrigger принадлежат
Base; локальные h-10/min-h-10 и мобильные копии не добавлять. Textarea,
ширина предметной панели, внешний margin и компоновка сохраняют свой контракт.
Канонический контракт — Base docs/platform/DESIGN_SYSTEM.md и docs/BRANDING.md.
UI/SDK/skills pins независимы; source-изменения не означают установку.

## Единое пространство PDLC

[Согласованный план](docs/plans/unified-spaces-20261019.md) вводит M:N и общий read admission. До полного
совместимого cohort это цель, не установленное поведение. Строгие V1/V2 и
applied migrations сохранять; ProjectRef/SpaceRef/V3 отдельны. Полный gate на
интегрированной вехе, в ходе реализации scoped checks. Runtime не активировать
по готовности одного продукта; не выбирать первый Project/Space неявно.


Контекст центрального запроса содержит immutable identity, свежую глобальную
роль, product scopes и исходный срок допуска. Не повышать роль в JIT и не
использовать исторический `is_system_admin` как доказательство роли платформы.
Task-local proof не передаётся в spawned jobs автоматически; потоки используют
явно захваченный исходный proof, а фоновые операции — свой ограниченный протокол.
Исторические уведомления и аудит не сохраняют доступ к данным отозванного
проекта. Ограничивать выборку и счётчики до pagination; неизвестный ресурс не
считать глобальным. События пользователей и system settings классифицировать
по их существующим глобальным правилам, отдельно от проектного содержимого.

Правила текущей транзакционной записи:
[PROJECT_CONTENT_AUTHORIZATION_V2](docs/PROJECT_CONTENT_AUTHORIZATION_V2.md).
Не использовать raw concrete connection как обход ordinary content guards.
Owner/reconcile и machine artifact/job ports имеют отдельные typed proofs;
read-only proof не становится правом записи. После owner-v2 adoption legacy
writer не считается совместимым даже до включения общего policy flag.

Для новых v2 machine context/recipient readers использовать authoritative
service_account factory и сохранять исходный proof до конца IO:
[PROJECT_SPACES_V2](docs/PROJECT_SPACES_V2.md#machine-context-reader-kind-revision-и-http-lifetime).
Legacy registered reader не заменяет эту проверку kind. Known policy revision
выбирает cache entry, но не является полномочием или owner revision.

## Подписи и карточки вариантов выбора

После проверки API в принятом UI pin используйте label или children Checkbox/Radio
для обычной подписи, appearance="card" — для варианта с рамкой и описанием.
Не копируйте label с border/rounded/padding и не оборачивайте такой выбор в
Card asChild. Base владеет hit target, focus, checked/disabled и нативной формой;
продукт — данными, правами и обработчиком. Внешняя grid/flex-компоновка допустима.
SSR использует checkbox_control/radio_control с appearance='card'; отдельный
click-handler или повторное input/change для оформления не нужен.
Контракт и пример: Base docs/platform/CHOICE_CONTROLS.md, pdlc-ui example Checkbox --case cards.


Видимый выбор файла использует `Input type="file"` Base: кнопка выбора,
focus и disabled следуют теме и branding. Native файловый диалог остаётся у ОС;
чтение, проверка содержимого/размера и отправка — у продукта. Скрытый технический
file input за Button допустим; видимый raw input не добавлять. Для обновления
оформления сверять UI pin; CLI/Telegram без web UI не получают новых зависимостей.

Подписи web-полей оформлять публичным Label; подпись/подсказку/ошибку одного
поля — FormField Base. Продукт сохраняет текст, htmlFor/id, required, данные
и обработчики; не копировать типографику подписи и фон/рамку/радиусы/padding
Input/Textarea. Внешняя компоновка, высота Textarea и моноширинный код допустимы.


## Композиция списков и страниц

Пагинация использует PageNavigation либо подходящий OffsetPageNavigation Base.
URL, курсоры, проверка допустимости страницы и callbacks принадлежат продукту.
Невалидное состояние блокируется disabled, запрос — pending; не создавать
фиктивные страницы или повторы кнопок. ResourceList linked-actions оформляет
сам anchor через interactive ResourceListRow, а независимое действие — рядом.
Сначала сверять наличие API по точному UI pin; локальные hover/focus/размеры
вместо общего контракта не добавлять.
