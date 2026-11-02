# Общие панели пространств и состояния уведомлений

## Кастомные поля Tracker: общий UI чтения — 25 октября 2026

Панель использует ResourceStateView, Fieldset и EmptyState Base вместо локального
текстового fallback. До первого ответа определения и значения не считаются
пустыми; успешный пустой каталог не ждёт ненужных значений. Временный отказ с
полным последним чтением оставляет данные и ввод, блокируя запись. 401/403/404
скрывают прежние поля; отказ доступа приоритетнее временного сбоя второго
ресурса. Продукт владеет этой зависимостью ресурсов; Base — отображением и
сохранением экземпляра children при ready ↔ partial-error.

Причина до переноса находилась в двух owner-границах: API reader терял status,
панель проверяла только data.length. Два GET используют уже существующий
generated контракт без устаревшего as-never, ApiError и InvalidResponseError.
Запросы, пути, четыре writer-функции, DTO, преобразования field values,
save payload и completion callbacks сохранены. Новые routes/API/DB отсутствуют.
Readback выполняет только два GET; неизвестный результат записи этой волной
не квалифицируется. Родительская форма создания сохраняет commit=change.

Fieldset и явный disabled закрывают также popup-контролы; guard предотвращает
запись поздним callback. Прочитанное значение обновляет только pristine text
draft. Локальная правка сохраняется и при новом server value; смена issue/project
пересоздаёт её в правильном контексте. Labels, поля и действия остаются продукту.

До исправления 18 из 19 новых regression cases воспроизводили ошибки. Итог:
60 distinct scoped cases в пяти файлах, включая реальный OpenAPI client с
тестовым fetch, оба прежних field suites и создание задачи. TypeScript, lint и
shell:check PASS. Повторные прогоны не складываются. Первый TypeScript FAIL
сохранён: причиной был прежний as-never на существующих typed GET путях.
Структурная сверка owner sources и шесть негативных контролей также PASS.

IAB: 288 контекстов (12 состояний × три темы × четыре ширины × две шкалы),
настоящие CustomFieldsPanel/ValueInput и переводы Tracker; resource hooks и
callbacks синтетические. Живые read failure/recovery, изменение server value,
сохранение blur, смена задачи, denial, закрытие disabled Select/MultiSelect,
обычный выбор и полный массив значений проверены. Нет overflow, console
warn/error и продуктовых HTTP requests. Все снимки 375×1100, 1920×1100,
2560×1440 полностью вмещают документ и просмотрены. Оба временных сервера
закрыты; final production hashes неизменны. Это не полный продуктовый backend,
authenticated acceptance или zoom200.

Первый QA resolver отдал CJS React без Vite-преобразования; диагностическая
фикстура не засчитана. Финальный сервер использовал зависимости consumer и
штатное разрешение модулей. Общий execution slot дважды отклонил busy запуск;
после освобождения обязательные scoped stages выполнены без обхода admission.

UI producer b50b7f53, reference e30f5c7e и семь pins прежние. Пакет, reference,
SSR/Auth fingerprints и окружения переиспользованы без generation/install/build.
SDK/skills не изменены. Evidence: private
`evidence/custom-field-resource-state-20261025/`. Publication, hosted CI,
установленный runtime и общий итоговый gate не заявляются.

## Общее меню аккаунта — 25 октября 2026

Локальное identity/theme/logout menu заменено AccountMenu. Nullish fallback имени, условие secondary email, logout.mutate/pending, modal=false и ACL/admin item сохраняются у Tracker.

UI producer `b50b7f53ea0b69f957b2babed8147129176fff03`, reference
`e30f5c7ee0b4690fd7f5f7df1caef73c1f478c70a1eb407db08ef90a14b09b1b`. Все семь UI pins согласованы;
шесть frozen offline refresh сохранили locks и React singleton. Независимые
SDK/skills pins не менялись. UI 0.3.2, reference schema 2 и API v1 сохранены.

Base: 16 distinct UI cases, 43 primitive guard cases, 72 reference cases,
types/examples/catalog/lint и настоящий offline UI-архив PASS. Шесть consumer
types и guards, четыре lint изменённых owners и 137 distinct consumer cases
(Admin 12, Tracker 38, Wiki 23, Forge 18, Fleet 25, AI Hub 21) PASS.
Whole-module/TS proof сохраняет остальную программу; шесть негативных контролей
проверяют pending, secondary, logout, route, ACL и AppShell forwarding.

IAB: 96 source-контекстов четырёх точных menu adapters, три темы,
320/375/1920/2560 и standard/large. Дополнительно проверены theme-only,
pending/ACL, owner links, однократный logout, modal/inert, клавиатура и возврат
фокуса. Это синтетические identity/actions без backend/SSO, не installed
acceptance. Source hashes неизменны; временный процесс закрыт, продуктовых
запросов и console errors нет. Снимки 375/1920/2560 имеют реальные размеры.

Evidence: private `evidence/account-menu-20261025/`. Архив: 822 файла,
1 149 993 байт, SHA256
`a03bb855d4b6865e9f11a8ccdb837a5a97e99f0e20f681b949a0712068223161`.
Source publication, hosted CI, общий integrated gate и установленная приёмка
этой scoped волной не заявляются. Предыдущие разделы сохраняют исторические
producer и результаты, а не описывают текущий UI pin.

Baseline — `db360b9b178482f5fd29d1b1ae007d822221f4dd`. Используются уже
доступные контракты UI pin `cf1e76e4be37f3611806f48b4d1b75d0470d78b0`.

Панели неподтверждённой команды и проекции Wiki, а также выбранного пространства
переходят на Card/CardContent. Собственная разметка loading/empty и пагинации
уведомлений заменяется LoadingState, EmptyState и PageNavigation Base.

Запросы, admission, сохранение набора пространств, CAS, operation key, readback,
сброс фильтра и URL уведомлений остаются у Tracker. Выбор checkbox с подробным
описанием требует отдельного общего контракта; этот план не объявляет его
мигрированным. Нативный file input вложений — технический контроль загрузки.

Проверки: scoped tests двух форм пространств и уведомлений, TypeScript и lint
изменённых файлов, guards Base. Полный интеграционный и браузерный gate — после
сведения следующей порции. Источники не означают установку; runtime не меняется.

Scoped результат: 31 test в трёх файлах прошёл; TypeScript, scoped ESLint,
shared choice/primitive guards и CLI check принятого UI прошли. AST трёх
production-файлов подтверждает сохранность логики вне JSX и критичных вызовов.
Заголовки и состояния формы связей также используют SectionHeader/Notice;
допуск и подтверждение Wiki не смешиваются с цветом или статусом успеха.

## Общие состояния чтения — 26 октября 2026

В 11 модулях 18 участков используют EmptyState, LoadingState и ResourceStateView
Base: комментарии, активность, репозитории задачи, трудозатраты, dashboard,
задача, уведомления, доска, определения полей, корзина и поиск. Пустота остаётся
под прежними условиями запроса; пагинация, фильтры, восстановление и удаление
сохраняют продуктовых владельцев. Загрузка имеет scope="local" и не становится
новым источником readiness общего host. Терминальные страницы сохраняют h1.

116 distinct cases в 11 выбранных файлах, TypeScript, scoped ESLint и UI contract
PASS. Проверка всех полных изменённых TS-модулей сохраняет остальную программу.
IAB проверяет реальные UI-фрагменты с синтетическими данными; это не приёмка
backend или установленного runtime. UI pin 430ef57512bca547e44f1a6d4c5878d52ee978ce
переиспользован; SDK/skills не менялись. Следующая отдельная работа — состояния
чтения LabelEditor и LinkEditor: pending/error ещё нельзя трактовать как пустоту.

## Метки и связи: подтверждённое чтение — 26 октября 2026

LabelEditor и LinkEditor используют ResourceStateView, EmptyState и Fieldset
Base. Три GET сохраняют HTTP status через ApiError; отсутствие массива в успешном
ответе — InvalidResponseError, не подтверждённая пустота. Labels ждёт оба чтения,
чтобы не предлагать уже прикреплённую метку. Повтор чтения не вызывает writer.
Последний полный snapshot при временном сбое сохраняется readonly вместе с
черновиком; 401/403/404 скрывают данные. Новый issue/project сбрасывает только
его локальную форму. Существующие API writers, DTO, paths и mutation payloads
сохранены; их семантическая сверка дополнена API/editor tests.

32 distinct scoped cases в четырёх файлах, TypeScript, scoped ESLint, format и
UI contract PASS. 21 причинный regression воспроизведён до исправления; две
первоначальные ошибки CSS-утверждений теста исправлены отдельно и не считаются
регрессиями продукта. IAB: 48 ready-контекстов, состояния чтения, keyboard,
readonly draft, retry только GET и однократные callbacks на настоящих редакторах
с синтетическими hooks. Backend и установленный runtime не проверялись.

Обнаружен отдельный прежний дефект ширины Base Select: width:100% перекрывает
caller w-auto. Локальный CSS workaround не добавлен; широкая компоновка формы
остаётся открытой до исправления Base и принятия нового UI producer.

## Ширина Select: принято исправление Base — 26 октября 2026

UI pin 6dca2182e838410fbaa6b56953b53c3b73265f6f закрывает выявленное перекрытие
w-auto. Исправлен только default ширины Base; классы формы и продуктовые
обработчики не менялись. Настоящий LinkEditor проверен в 12 сочетаниях IAB:
три темы и 320/375/1920/2560, desktop-контролы стоят в одной строке, overflow нет.
Выбор типа связи не записывает данные; подтверждение вызывает один прежний
payload. Окружение чтения/записи синтетическое, без backend и Auth.
