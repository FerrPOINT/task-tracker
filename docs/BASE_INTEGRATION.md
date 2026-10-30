# Интеграция task-tracker с Base

## Текущий source-кандидат UI0.3.32

Producer `6f3c557698a8224c5a19e56bdc37b199ba076694`, reference
`3d36261c8fa88dcb116b0efbd03596237702f265aa479bbb5758bc2a5d4ba701`.
Frozen/offline preparation подтвердила реальный пакет0.3.32 и CLI integrity;
внешние resolutions и независимые SDK/skills pins сохранены. Base Textarea
получил общий пересчёт autoSize при настоящем увеличении текста браузера.
Native browser регрессия autoSize, полный Base unit и source/package checks
прошли. Product frontend build/unit и IAB остаются отдельными gates;
новый pin сам по себе их не подтверждает. Установленный runtime не обновлялся.

## Предшествующий source-кандидат UI0.3.30

Producer `d12f12e811282cadf881b01e2acd210b3561e88e`, reference
`0127aeb8c91ade94906908a0d0f7b66dea77e5129b360ebfbaf6617826b70bca`.
Общий resolver шрифта используется tokens, React/SSR знаком и Canvas favicon.
Образец вне BrandingScope получает шрифт своего документа; favicon ждёт загрузки
нужных символов выбранного встроенного семейства. Публичные API сохранены.

Frozen offline preparation и CLI integrity подтвердили фактический пакет0.3.30.
Внешние dependency resolutions, SDK/skills pins и продуктовые обработчики не
менялись. Base проверен из настоящего UI-архива вне workspace без Storybook.
Новые production frontend builds, hosted CI и продуктовая браузерная приёмка
учитываются отдельно; установка исключена. Доказательства другого приложения
или предыдущего UI pin не объявляются приёмкой этого продукта.

## Предшествующий source-кандидат UI0.3.29

Producer `b26cb497e4bdf797875301d5a2aa58f156cdd571`, reference `81c68bc7c8b473216b02de86731a786fe2ee5b8b0354aa70949aa514e6bffcc4`.
Исправлен общий runtime favicon: PNG получает правильный MIME, cleanup возвращает
исходные href/type. Для внешнего legacy URL тип определяет ответ. Публичные API,
SDK/skills pins и продуктовые обработчики сохранены. Source selection, frozen
preparation, artifact checks и IAB — отдельные доказательства; установка исключена.

## Предшествующий source-кандидат UI0.3.28

Точный UI SHA указан в [pin](../.base-ui-revision):
`7ad53fbee14c9c8df38d5aa17acd553288ef19d9`, `@pdlc/ui`0.3.28,
reference `2a78538e33c78db0d713ff81d4f4ff0c143a0783ea831c2b44238894ac36efe3`.
Base #384 содержит общий React/native SSR Toaster offset над fixed actions и
сохраняет независимые card/control/menu/dialog радиусы. Расчёт, skin и механизм
принадлежат Base; продуктовые данные, права, записи и callbacks остаются здесь.

Объявленный source pin, frozen preparation, build и браузерная приёмка — разные
этапы. Frozen offline install и CLI integrity нового snapshot PASS: версия0.3.28
и reference совпали с producer, внешние dependency resolutions сохранены.
Build и product browser gates нового комплекта ещё открыты; исторический
PASS0.3.27 ниже не переносится на него автоматически.
SDK/skills pins независимы. Установка и authenticated runtime этим документом
не подтверждаются; полная field/app матрица остаётся открытой.

## Квалификация прежнего UI0.3.27: независимые радиусы контейнеров


Текущий pin включает общий card-radius для Card, framed ResourceList,
Disclosure, preview frame и toast. Вложенные controls, меню и диалоги
сохраняют отдельные control/menu/dialog. Расчёт и skin принадлежат Base;
локальные копии оформления не добавлены.

Frozen offline install, CLI integrity, exact installed snapshot, TypeScript,
shared-shell contract и production build PASS на этом UI pin. SDK/skills pins
и внешние dependency resolutions сохранены. Consumer TS/JS и конфигурации
не изменились относительно собственного UI0.3.26 gate (с учётом исправления
Fleet prepend); прежние unit results переиспользованы по входам, а не объявлены
новым полным прогоном. В Base заново прошли8 native branding и9 native CSS
cases, в том числе три темы/оба порядка CSS и разные card/control значения.
Private evidence: ui027-card-radius-20261029/gate и unit-input-deltas.json.
Установка, hosted CI и полная authenticated browser matrix этим не подтверждены.

## Предыдущие source-квалификации


Финальный checkpoint 2de0f72b включает bounded fetch/body wait. Frozen offline
refresh, exact snapshot, types, production build и shell contract PASS.
Provider/продуктовые композиции относительно d7c1d8c3 неизменны; нижние auth cases
переиспользованы, не объявляются новым прогоном. Changed transport покрыт 96 Base
auth cases; исходный Admin integration file прошёл 18 cases на final pin.

Первый checkpoint d7c1d8c3: P1 clock-skew исправлен в Base, независимый JWT deadline использует owner clock
и monotonic time, не Date.now до owner readback. Frozen offline install,
exact installed snapshot, TypeScript, shell contract и production build PASS;
affected application-access suite: 4 PASS. Production TS, API, SDK/skills,
grants и runtime не менялись. Отсутствующий route manifest не придумывался;
полный route CLI не заявляется. Независимое review требуется до ready/merge;
stand и PM03 pin не затронуты, остальная theme/browser matrix — follow-up.

Предыдущая квалификация 0.3.25: frozen install, точный installed snapshot,
TypeScript, shell/primitive/CSS guards, lint, format, OpenAPI check, build
и 74 scoped tests /7 файлов PASS. Только formatter исправил 18 файлов;
emitted JavaScript AST до/после совпадает. SDK/skills pins и lockfile не менялись.
Полный продуктовый suite, authenticated UI, установка и полная тематическая
матрица этим source-проходом не объявляются завершёнными.

Совместимый UI-кандидат выровнен с остальными потребителями; предметные компоненты Tracker этим обновлением не заменяются.

## Исходная UI-поставка — 21 октября 2026

UI был закреплён на `94272dc5512be58ec6c88fb7c1d1d33c7599a3e4` ([pin](../.base-ui-revision)); зависит от [Base #297](https://github.com/FerrPOINT/services-base/pull/297).
Проверки consumer и устранение прежнего локального offset логотипа описаны
в [отчёте квалификации](reports/ui-repin-qualification-20261021.md).
SDK и skills pins независимы и этим обновлением не меняются. Общая типографика,
плотность и отдельные радиусы применяются в Base к React, Portal и SSR;
продукт сохраняет данные, права, запросы и действия. Штатные SSR-ресурсы Workflow
обновлены из того же UI SHA. Это source-кандидат, а не установка платформы.

Датированные разделы ниже описывают историю квалификаций. Текущий pin сам по себе не подтверждает установку или authenticated-приёмку.

Активный Base SHA закреплён в [`.namespace-base-revision`](../.namespace-base-revision).
При отсутствии этого файла CI и `scripts/build.py` используют legacy
[`.base-revision`](../.base-revision). `main` Base не является воспроизводимой
зависимостью. Checkout Base должен лежать соседним каталогом
`services-base`. Перед standalone build выполнить из корня продукта:

```sh
pin=.base-revision
if [ -f .namespace-base-revision ]; then pin=.namespace-base-revision; fi
python3 ../services-base/scripts/verify_base_revision.py --base ../services-base --revision "$pin"
python3 scripts/build.py
```

Docker standalone и umbrella используют соседние checkout как build context.
CI получает Base на том же SHA. Forge runner sources формируются штатным
`materialize_runner_sources.py` из чистого delivery candidate, с manifest происхождения.

Rust build toolchain: 1.88.0; Node: 22.20.0; pnpm: 10.28.1.
Cargo использует `--locked`; pnpm — `--frozen-lockfile`. MSRV проверяется отдельно.
Приёмка включает OpenAPI drift/compatibility, package consumer и effective-theme
проверки вместе с продуктовыми тестами. После frontend build запустить preview
и `pnpm theme:check http://127.0.0.1:4173`.

Base содержит технические механизмы; продукт сохраняет авторизацию, PAT scopes,
данные, ORM, миграции, HTTP статусы/коды/поля, env/defaults и навигацию.
Новых миграций БД в этой задаче нет. Откат — предыдущий проверенный набор SHA/images.

Закреплённый SDK включает `ServiceBridge::try_token_with_name` из Base PR #142.
Имя поступает из проверенного session/PAT ответа, не из неподписанного payload
или части email. Опубликованный каталог пользователей из Base PR #143 сохраняет
существующий массив ответа и совместим с этим потребителем.

Текущая реализация и доказательства проверок учитываются в реестре Base
`docs/plans/base-unification-register.json`; незавершённые гейты не отмечаются пройденными.

Кнопка входа вызывает `beginSso(..., { interactive: true })`; автоматический
guard сохраняет неинтерактивный вызов. `isSsoNavigationInterruption` из Base
различает pending logout/отмену навигации и настоящий отказ Auth. Task Tracker
снимает блокировку кнопки при завершении или отмене promise. Identity, PKCE,
state, nonce, API и машинная авторизация не меняются. Source-регрессии покрыты
login-тестами; локальные fixture-проверки не заменяют живую SSO-приёмку.

Закреплённый Base сохраняет focus внутри общего `ConfirmDialog`, когда запрос
переводит все действия в pending: Tab/Shift+Tab остаются в диалоге, Escape не
закрывает незавершённую операцию. После её завершения действуют прежние правила
закрытия и возврата focus. Продуктовые операции и HTTP-контракты не меняются.
Обновление pin требует повторных frontend/package/theme и живых UI-проверок;
исторические результаты прежнего Base не подтверждают эту приёмку.

## Отдельный UI pin и размещение в Base

`.base-ui-revision` закрепляет frontend Base; `.base-revision` остаётся SDK pin.
Frontend job CI проверяет UI checkout независимо от backend jobs. Для frontend
сборки используйте `python scripts/build.py frontend` и соседний UI checkout
на точном SHA. Совместную пересборку backend и UI с разными pins выполняйте
отдельно, сохраняя исходный SDK checkout.

`VITE_PLATFORM_ORIGIN` передаётся как Docker build argument. Пустое значение
сохраняет standalone; настроенное включает memory router, silent SSO и общий
bridge. Продукт сохраняет собственные API, профиль, permissions и формы.
[Оболочка Base](https://github.com/FerrPOINT/services-base/blob/2523e7ac1b56a14e9038f41472df4ce7a114dc1e/frontend/platform/README.md) описывает
конфигурацию, подготовку кадра и отдельную runtime-приёмку.

## Реорганизация source Base

Общие Rust SDK находятся в `services-base/base-modules/rust`, Python SDK —
в `services-base/base-modules/python/cli-core`, UI — в `services-base/base-modules/ui`.
Auth, оператор, host и справочник имеют собственные корневые каталоги приложений.
Имена пакетов, HTTP/wire-контракты, данные и credentials этим переносом не меняются.
UI/SDK/skills pins независимы; source-комплект проверяется по точным revisions.
[Карта Base](https://github.com/FerrPOINT/services-base/blob/a4dea438ec99103ca83b51540d11de548b1ee82d/docs/platform/ARCHITECTURE.md)
и [ADR 0006](https://github.com/FerrPOINT/services-base/blob/a4dea438ec99103ca83b51540d11de548b1ee82d/docs/adr/0006-base-repository-layout.md)
определяют расположение и направление зависимостей. Установка выполняется отдельно.

## Каталог UI

UI pin принимает Base #245. При недоступном каталоге
`usePlatformServices().services` пуст: продуктовые ссылки показывают штатное
недоступное состояние. Меню сохраняет подписи без неподтверждённых переходов;
известные адреса из runtime/host работают независимо от health. Восстановление
каталога использует прежний интервал. SDK и HTTP-контракты не меняются.

## UI-кандидат 2026-10-18

Общий UI закреплён на `6f1c797a0b11380f7e36db5ced0360f25a0265a5`. SDK и skills pins остаются независимыми.
Создание первого проекта использует существующий ProjectFormDialog; Task Tracker сохраняет права, API, каталог и ввод задачи.
Проверки source-кандидата и установленного PDLC2 учитываются отдельно;
обновление pin не означает установку.

## Общие строки действий — 2026-10-26

UI producer: `c0d2245e40303efffc04c7d8c24eb3746afd1439`. В Base добавлен ActionGroup и SSR action_group;
PageHeader, SectionHeader, PageNavigation и ChatComposerActions используют общий
каркас. Данные, права, курсоры, submit/pending, неизвестный результат записи и
readback сохраняются у продукта. Для пояснений используется ResourceListDescription.
Внешние margin/width/grid placement и sticky-контейнер принадлежат экрану;
общие gap, разделитель и мобильная компоновка — Base. Нативные формы,
тип кнопок, hidden, IDs и callbacks сохраняются. SDK и skills pins не изменены.

Проверены шесть offline frozen installs, singleton React, CLI/reference,
frontend types, guards и штатный SSR fingerprint. Изменённые React-сценарии
имеют scoped regression checks; общий UI проверен в IAB на трёх темах и
ширинах320/375/1920/2560. Это source adoption; установленный runtime, SSO
и продуктовые данные не изменялись. Полный release/hosted/runtime gate отдельно.

## Строки действий — 2026-10-26

1Количество строк действий, переведённых на ActionGroup Base: 3.
Внешнее размещение, видимость и предметные grid остаются у продукта; gap и
перенос — у Base. Callbacks, ref, права, native типы, URLs и сохранение
не меняются. UI producer остаётся c2ca61094a904706fceeda8eb8f3b3cfd2e7cdfc;
UI API, SDK/skills pins, зависимости и установленный runtime не обновляются.
Scoped проверки и компонентный IAB не заменяют полный integrated gate
и authenticated-приёмку страниц.

## Вертикальные группы действий — 2026-10-26

Принят UI producer `7294a7974a898f182c22034b511c6b4b15205cef`; SDK/skills pins сохранены.
Совместимый `ActionGroup orientation="vertical"` и SSR `action_group`
описаны в [контракте Base](https://github.com/FerrPOINT/services-base/blob/7294a7974a898f182c22034b511c6b4b15205cef/docs/plans/action-orientation-20261026.md).
Проверены архив UI, offline frozen install шести React-потребителей,
singleton React, CLI/reference, шесть frontend types и SSR fingerprint.
Forge дополнительно прошёл19 адресных тестов и lint двух изменённых модулей;
остальные backend/application builds не выполнялись.

IAB проверяет source-компонент Forge с синтетическими callbacks:24 сочетания
темы, ширины и направления; это не приёмка установленного приложения.
Данные, права, pending и подтверждения остаются у продукта; сетка корзины
Tracker связана с её данными и не получает общий breakpoint из Base.
Публикация, полный интеграционный gate и установка учитываются отдельно.

## Подготовленный HTML и клавиатура — 2026-10-27

Принят совместимый UI producer `a95d9fe7b31cca23e1f6432bae43246120859efb`; SDK/skills pins сохранены.
Общий Prose keyboardScroll документирован в [Base](https://github.com/FerrPOINT/services-base/blob/a95d9fe7b31cca23e1f6432bae43246120859efb/docs/platform/DESIGN_SYSTEM.md).
Проверены настоящий архив UI, offline frozen install, singleton React,
reference/CLI и typecheck данного frontend.
Backend build, hosted CI, общий интеграционный gate и установка здесь
не выполнялись; source adoption не меняет установленный runtime.

## Markdown и защита ухода — 2026-10-27

CommentBody использует CodeBlock Base для fenced code; parsing, skipHtml и
политика ссылок сохранены. Стабильный renderer не пересоздаёт pre при повторном
рендере. Прошли 4 адресных теста, typecheck, lint и shell guard.

Используется уже принятый UI producer
`a95d9fe7b31cca23e1f6432bae43246120859efb`; UI/SDK/skills pins и зависимости
не менялись. Production bytes UI совпадают с ранее проверенным архивом,
повторные install/pack/generation не выполнялись.

IAB проверяет source-компоненты на синтетических данных. Это не установленное
приложение, API/Auth или runtime-приёмка. Снимки 375/1920/2560 сохранены
отдельно от основной матрицы трёх тем и ширин 320/375/1920/2560.
Полный интеграционный gate, hosted CI, push и установка не выполнялись.

## Общая подпись навигации — 2026-10-27

Принят совместимый UI 0.3.4, producer `efa4babe176b42530a3e17fc25f10e8678f82d33`.
Подписи боковых ссылок используют SidebarItemLabel Base; native span,
полное доступное имя, title, маршруты, состояние и callbacks сохранены.
Ellipsis и compact visibility принадлежат Base; private CSS-класс не копируется.
Проверены настоящий архив, offline CLI и frozen adoption: шесть React consumers
с одним React, седьмой SSR consumer — через штатный export/check. SDK/skills
pins неизменны; это source-кандидат, без установки и hosted CI.

Итоговый producer проверен 38 scoped tests боковой навигации;
TypeScript, lint изменённых модулей и shell contract gate прошли отдельно.

## Нативные атрибуты SSR Button — 2026-10-27

Текущий UI 0.3.5: `8bdd8ecd54eb6a6f553bfbadcfaf07c99f4a801d`. Сохраняются React API и независимые SDK/skills pins.
React/JS/CSS и типы API не менялись; frontend/backend не пересобирались.
Новые SSR параметры доступны в этом точном Base, не являются новым React API.
Архив UI и offline frozen adoption проверены; общий интеграционный gate,
hosted CI, публикация и установка здесь не выполнялись.

## Общие нативные элементы SSR — 2026-10-27

Принят совместимый UI 0.3.6, producer `623deb7da5813fa9d1185ded7688c35bb15cd43e`.
Изменения принадлежат SSR macros и справочнику; React API, JS/CSS/types и
зависимости совпадают с 0.3.5. Повторная frontend/backend-сборка не нужна при
неизменных inputs. Настоящий архив из 868 файлов сверён с установленным UI,
offline frozen install, singleton React и CLI check пройдены. SDK/skills pins
неизменны. Это source adoption, без hosted CI и установленного runtime.

## SSR композиции и native reset — 2026-10-27

Текущий UI 0.3.8, producer `cf88f7094044a403e6bb7105cfa88b12c4208c96`.
Общие SSR parts принимают native attributes. В отдельном select-runtime
исправлено обновление trigger после нативного reset; API React и CSS/types
не менялись. Root UI/lib не импортируют этот runtime, прямых импортов
select-runtime в React runtime source продукта нет. Настоящий архив из
868 файлов сверён, offline frozen install, singleton React и CLI пройдены.
SDK/skills pins неизменны. Это source adoption, без установки и hosted CI.

## Рабочая область Base — 2026-10-27

Текущий UI 0.3.9, producer `bab93d2683367117784c2f01fcbed7f80fd3ef15`.
Workbench и семь частей добавляют общий каркас рабочей области внутри PageFrame:
высота по настоящим gutter, внутренний scroll, адаптивные Aside/AsideAction и
mobileFlow для длинной формы. SSR-аналога этого React-контракта нет.
Настоящий архив из 882 файлов сверён с установленным UI; offline frozen install,
singleton React и CLI check пройдены. SDK/skills pins сохранены. Это source
adoption; приложение, backend и Storybook не пересобирались, runtime не менялся.

## Подтверждение требований

Группа подтверждения SdlcPanel использует Fieldset separated Base с доступным
именем и визуально скрытой legend. Группа не получает общий disabled: права,
acknowledged pin, pending и готовность policy проверяются прежними отдельными
controls. Технические ссылки ролей оформляются ResourceList/ResourceListRow;
native section и иерархия заголовков сохраняются.

Tracker владеет revision/hash, routing policy, idempotency/readback и командой
подтверждения. Оформление группы не создаёт событий и не меняет данные.
Проверки source не означают runtime-приёмку; UI/SDK/skills pins сохранены.

## Общие секции и полосы данных — 2026-10-27

UI 0.3.10, точный producer `94f2ccd9e6f09eba9fbf67f4582a9fa7a91c44b9`.
Прозрачные области dashboard, создания/карточки задачи, связей, настроек
проектов и уведомлений используют ContentSection. URL, выбор проекта, dirty
tracking, права и обработчики сохраняются у Tracker.

Фирменные разделители, inset и плотность принадлежат Base. Данные, DTO,
права, формы, смысловые статусы и внешний grid/gap остаются у продукта.
В справочнике доступны самостоятельные ContentSection/Form и DataBar/basic;
для статических данных применяется DataBar, для выполнения операции — Progress.

Архив 895 файлов и установленный UI сверены; offline frozen refresh, React
singleton, CLI check, TypeScript, UI contract guard и scoped tests пройдены.
Изменённые TSX также прошли scoped ESLint.
Браузерные доказательства относятся к изолированным общим композициям,
не ко всем страницам приложения/API/Auth. Полный интеграционный gate и
установленная приёмка остаются отдельными. SDK/skills pins и runtime сохранены.

## Общие метки и inline-код UI 0.3.11 — 2026-10-27

Точный UI producer `755c9f9a27ba81ddd841b43c511bb372a3c93e40` принят отдельно от SDK/skills.
Три счётчика dashboard используют Badge с прежними подписями, нулём и formatter. Выбранные семь dashboard tests прошли.

Настоящий архив UI содержит 900 файлов; все production bytes сверены с
producer и шестью установленными frontend-пакетами. Offline frozen refresh,
React singleton и CLI check пройдены; runtime и зависимости backend не менялись.

IAB evidence относится к пяти изолированным композициям из настоящих owner/Base
исходников: 60 сочетаний темы/ширины, 18 branding-пресетов, три статуса Wiki,
readonly click и Enter/Space. Это не приёмка всех экранов, Auth или API продукта.
Полный интеграционный gate остаётся отдельной открытой стадией.

## Общая render-граница UI 0.3.12 — 2026-10-27

Точный UI producer `20633a6489dfd33e7afa782e0a06c23faea26b7a` принят отдельно от SDK/skills.
ProjectCommandRecovery — тонкая композиция общего RenderBoundary с прежним ErrorState. Identity keys обоих вызывающих экранов, journal, DTO и handlers не менялись. Новый тест использует настоящий useSessionCommand: две исходные команды переживают render-сбой без записи/удаления storage и автоматического повтора; prop changes не сбрасывают ошибку, явный key восстанавливает область.

Настоящий UI-архив: 904 файла, offline CLI/reference и production bytes
проверены вне workspace без Storybook. Шесть frontend-пакетов сверены с архивом;
cached offline frozen refresh, React singleton и CLI check прошли.
Общий механизм и ограничения: Base docs/platform/DESIGN_SYSTEM.md,
раздел «Граница render-ошибки React». Данные, fallback, отчёт и reset identity
принадлежат владельцу. HTTP/async/event/SSR ошибки обслуживаются отдельно.

IAB evidence относится к фактическим Base/CatalogProviders source-композициям:
12 сценариев ввода/сбоя/reset и 12 длинных fallback в трёх темах и четырёх
ширинах; сохранены preview ready/error и черновик. Это не приёмка установленного
продукта или Auth/API. Интеграционный gate, hosted CI и публикация остаются
отдельными стадиями. Runtime не менялся.

## Общая responsive-подписка UI 0.3.13 — 2026-10-27

Точный UI producer `83c961379b25379e8c83ebc207eec60cfb4ebbef` принят независимо от SDK/skills.
IssueDetailLayout использует useMediaQuery вместо локальных getDesktopLayout/subscribeDesktopLayout. Query 1024 px, grid, sticky aside, порядок слотов, запросы и формы сохранены. 62 shell/issue tests прошли, включая сохранение несохранённого ввода при переходе через breakpoint.

Новый hook доступен через `@pdlc/ui/lib`; query задаёт владелец. Необязательный
callback выполняется до React layout update только на browser event, сохраняя
перенос focus из исчезающего control. Mount/query changes не запускают действие.
SSR/отсутствующий matchMedia дают false. Контракт, cleanup и standalone example
доступны через `pdlc-ui show useMediaQuery` и `pdlc-ui example useMediaQuery`.

Архив 907 файлов, offline CLI, шесть frozen refresh, React singleton и сравнение
установленных production bytes прошли. Lockfiles после подготовки не менялись;
backend, SDK/skills pins и установленный runtime не изменялись.
IAB source proof: 27 Tracker viewport transitions, 30 shell/drawer checks и
12 recovery cases в трёх темах. Ширина менялась без reload; ввод, focus и journal
сохранены. Это source-композиции, не приёмка полного authenticated приложения.
Прежний IAB attachment gap закрыт новым успешным run; ошибки сохранены в истории.
Полный интеграционный gate, Rust transport vectors, hosted CI и публикация отдельно.

## Общая геометрия истории UI 0.3.14 — 2026-10-27

UI producer `f3b99e3ce15a4a447d58790f118c6be70e931a18` принят независимо от SDK/skills.
Архив содержит 910 файлов; offline CLI, шесть frozen refresh и production bytes
проверены. В Workflow штатный SSR export остался byte-identical. Это обновление
source-поставки; установленный runtime не менялся.

Продуктовые представления не менялись. Новый hook доступен для подтверждённых
историй с prepend/live append через публичный @pdlc/ui/lib. Перед использованием
прочитать карточку и самостоятельный пример; готовый WorkbenchScroll/CodeBlock
не заменяется локальной визуальной обёрткой. Обычный scrollIntoView остаётся
нативным действием владельца.

Base qualification: 17 hook tests, 69 primitive checks и 37 search assertions.
IAB Base/Forge source-compositions: 24 theme/width cases плюс три intent/error
cases; это не полная authenticated-приёмка продуктов. Полный интеграционный gate,
Rust search transport, hosted CI и публикация остаются отдельными стадиями.
