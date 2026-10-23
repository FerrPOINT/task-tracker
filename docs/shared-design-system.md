# Подключение общей дизайн-системы

Продукт использует @pdlc/ui 0.3 и опубликованный документ Admin. Общие controls,
цвета, шрифты, плотность, скругления и popup принадлежат Base; запросы,
права, данные и callbacks сохраняются здесь. UI/SDK/skills pins независимы.

Канонический контракт: services-base/docs/platform/DESIGN_SYSTEM.md.
Компоненты искать через pdlc-ui search → show → example. Не создавайте
копии примитивов, собственный Toaster и глобальные определения Base tokens.
Существующие source-компоненты и preview получают общий UI; исторические
прототипы не являются установленным приложением. Установка выполняется отдельно.

Источник UI 0.3.0: `579749e19e9088be200c62e5563a3cb2db4d8c9f` в `.base-ui-revision`.
SDK и skills pins независимы; установка этим source-кандидатом не выполняется.

Vite plugins включает `createUiFontAssetsPlugin()` из `@pdlc/ui/build`: notices
встроенных fonts попадают в dist через общую реализацию Base.

## Общие сообщения и формы — source-кандидат 2026-10-22

Сообщения операций используют Notice; независимые состояния чтения — LoadingState
и ErrorState. В редакторах задачи, учёта времени и настроек проекта оформление
label/hint/error принадлежит FormField; заголовок задачи использует PageHeader.
Валидация RHF/Zod, таймер, dirty state, DTO, права и callbacks остаются у Tracker.
Обязательность полей сохранена: summary проверяет RHF, название проекта сохраняет
native required. Ошибка записи не очищает введённое значение и не повторяет запись.

Управляемые Dialog/AlertDialog получают возврат фокуса из Base без локального hook.
Явный returnFocusRef/fallbackFocusRef и onCloseAutoFocus остаются доступными.
Проверка фокуса ожидает завершение lifecycle закрытия.

Текущий перенос: 35 композиций в 16 файлах. Последние scoped результаты покрывают
150 cases, после общего исправления диалогов повторены 32 затронутых case.
IAB source QA: 72 сочетания шести сцен, трёх тем и ширин 320/375/1920/2560;
24 возврата фокуса, сохранение черновика и привязка ошибок к полям подтверждены.
Синтетический PATCH отклонён локальным QA-процессом; продуктовый backend,
credentials и установленный runtime не использовались. Это source-доказательства;
итоговый UI SHA, архив и интеграционный gate ещё не закрыты.

## Заголовки интерфейсных секций — 2026-10-22

Ещё 11 секций в семи source-файлах используют SectionHeader Base: метки и связи
задачи, Admin, dashboard, создание задачи, пользовательские поля и общие настройки
проекта. Три прежние flex-композиции заголовка с действиями заменены actions slot;
уровни h2/h3, ссылки, обработчики и id configured-fields-title сохранены.
Шрифт, цвет, перенос длинного заголовка и плотность задаёт Base.

Заголовок, полученный из текста описания задачи, и название записи уведомления
остаются предметным содержимым. Остальные h1 и page-композиции проверяются отдельно.
AST подтверждает сохранность non-render logic и event/form/URL/ARIA контрактов.
Scoped unit — 80 PASS в восьми затронутых файлах; TypeScript, scoped ESLint,
choice/primitive и shell guards прошли. Новая браузерная приёмка этих страниц
остаётся частью итогового UI gate; прежняя проверка SectionHeader не выдаётся
за новую проверку продукта. Архив, exact pins и installed runtime не изменены.

## Активные шапки страниц — 2026-10-22

11 h1 в десяти файлах переведены на PageHeader. Название, аватар/навигация,
контекст и действия передаются через публичные slots; локальные flex-шапки
удалены. Параметры URL, обработчики, формы, refs и запросы не менялись.
Старая RegisterPage не используется маршрутизатором: /register ведёт на /login.
Её прямые unit fixtures остаются legacy, а не действующим входом в приложение.
Кроме неё raw headings остаются только в пользовательском тексте задачи и
названии предметной записи уведомления.

Scoped unit — 85 PASS в девяти файлах; TypeScript, scoped ESLint, choice и shell
guards прошли. IAB проверил четыре реальные страницы на синтетических GET:
каталог проектов, настройки с длинным именем, dashboard и отсутствие project
context. 48 сочетаний трёх тем и 320/375/1920/2560 без overflow; размер h1
совпадает с действующим token. Создание из шапки открывает прежний Dialog;
cancel возвращает focus на trigger, отправка не выполнялась. Full-page screenshots
375/1920/2560 проверены. Остальные страницы имеют unit-покрытие; полная итоговая
UI-приёмка и package/pins остаются открытыми.
## Общие строки dashboard — 2026-10-22

Dashboard уже использует ResourceListRow Base. Два оставшихся rounded-sm на
внутренних ссылках удалены: радиус, hover и focus задаёт сама строка Base.
URL задач и досок, проектные данные и компоновка не менялись. Шесть dashboard
cases, TypeScript, scoped lint, choice и shell guards прошли. Общая поставка
и новый продуктовый browser gate остаются отдельными этапами.

## Метки задач и наблюдателей — 2026-10-22

Шесть локально оформленных меток в пяти файлах используют Badge Base: остаток
наблюдателей, тип задачи в detail/board, статус dashboard и ключ/статус результата
поиска. Подписи, идентификаторы, фильтры и действия остаются у Tracker; оформление
метки определяется Base. Компактный счётчик поверх колокольчика перенесён
следующим этапом на Badge appearance=counter, как описано ниже.

52 scoped unit cases, TypeScript, scoped lint, choice и shell guards прошли.
AST-сверка подтверждает сохранность non-render logic, событий, URL и form/ARIA
атрибутов. Продуктовый browser gate и конечная UI-поставка остаются открытыми.

## Журнал, списки и переходы страниц — 2026-10-22

Две записи activity используют Card/CardContent, desktop worklog — Card с
настоящей Table без лишнего padding. Журнал аудита и корзина используют
ResourceList/ResourceListRow; структура list/listitem и article, keys и действия
сохранены. Подтверждённая пустота каталога и отсутствие совпадений используют
EmptyState с разными предметными подписями. Поиск и корзина используют
PageNavigation: page, hasNext, pending, диапазоны и URL принадлежат Tracker.

73 scoped unit cases в семи файлах, TypeScript, scoped lint, choice и shell
guards прошли. AST-сверка подтверждает неизменность non-render logic и
поведенческих атрибутов, кроме двух явно проверенных адаптеров пагинации.
Сценарии сохраняют позднюю страницу, посторонние URL-параметры и восстановление
после пустой страницы. Новый продуктовый browser gate остаётся открытым.

## Режимы, счётчик и общий вход — 2026-10-22

Четыре группы выбора режима используют RadioGroup appearance=segmented:
режим поиска, фильтр уведомлений, плотность и мобильная колонка доски.
URL, значения фильтров, действия и DnD остаются в Tracker. Счётчик уведомлений
использует Badge appearance=counter, а локальные notification-count CSS и
исключение для светлой темы удалены. Число, скрытие нуля и положение у иконки
сохраняет Tracker; Base не ограничивает значение числом 99.

Неиспользуемые RegisterPage и её тест формы удалены. Тест настоящего маршрута
проверяет /register → /login с replace и возврат Back на предыдущую страницу;
настоящий SSO не запускается. Backend registration API не изменён.

Режимы подтверждены 30 scoped cases в трёх файлах; счётчик и redirect —
18 cases в двух. TypeScript, scoped lint и choice/shell guards прошли.
Source-проверка использует существующую private qualification config;
согласованный UI pin, настоящий архив/peer graph и продуктовая browser-приёмка
остаются финальным gate.

## Общие рамки доски — 2026-10-22

Карточка задачи использует Card asChild surface=raised с настоящим article
draggable и CardContent. Колонка использует Card highlighted и общие
CardHeader/CardContent. Внутренние отступы default/compact задаются Base,
разрешённые переходы, dataTransfer, WIP, responsive status и mutation остаются
в Tracker. В карточке сохраняются отдельная ссылка и меню; контейнер не становится
второй кнопкой. Кнопка создания пока сохраняет локальное dashed-оформление и
включена в следующий, более широкий аудит классов поверх Base.

Семь scoped board cases включают допустимый/запрещённый drop, отсутствие mutation
до drop и единственную mutation при разрешённом переносе. AST-сверка подтверждает
неизменность non-render logic и 29 owner interaction/URL attributes. Общая Card
проверена отдельно в Base/SSR/IAB. Продуктовая browser-приёмка и поставка остаются
отдельными gates.

## Опасные действия и контраст предметного цвета — 2026-10-22

Восемь действий в шести файлах используют штатные Button destructive-outline,
size=icon и DropdownMenuItem tone=danger. Удалены копии цвета, рамки, hover,
focus и размеров; идентификаторы, confirmation, mutations и callback-порядок
не менялись. AST-сверка входит в общий реестр 17 действий трёх продуктов.

Локальный расчёт WCAG для меток заменён readableForeground из @pdlc/ui/lib;
labelForeground остаётся alias для совместимости. Палитра, создание, привязка
и повтор attach после частичного сбоя остаются у Tracker. Геометрия цветных
меток и их вложенные действия ещё требуют общего контракта Base.

60 scoped cases семи файлов и четыре LabelEditor cases прошли; TypeScript,
scoped lint и новый primitive gate прошли. IAB проверил общие Button/Menu,
а полный продуктовый browser gate, точный UI pin и package peer graph открыты.

## Цветные метки и общий предметный цвет — 2026-10-22

Этот этап закрывает отмеченный выше остаток геометрии LabelEditor. Метка
использует Badge subjectColor, привязка — Button variant=subject, удаление —
отдельный Button size=icon с наследуемым цветом. Удалены локальные палитра
текста, рамки, радиусы, hover/focus и размеры контролов. Палитра значений,
проверка #RRGGBB, ID, create/attach/detach, сохранение имени и повтор attach
после ошибки остаются у Tracker. AST подтверждает прежнюю прикладную логику
и 29 owner attributes; labelForeground остаётся совместимым alias Base.

Пять scoped unit cases, TypeScript, scoped lint и primitive gate прошли.
Общие Badge/Button проверены через IAB: три темы, четыре ширины, контраст,
длинное содержимое, отдельное действие и возврат focus. Синтетическая сцена
подтверждает смену шрифта, масштаба и радиусов без изменения предметного цвета
или вызова записи. Это не продуктовый browser gate и не installed runtime.
Точный UI pin, реальный архив/peer graph и итоговая приёмка ещё открыты.

## Общие ссылки-действия — 2026-10-22

Две ссылки полного списка dashboard используют Button asChild variant=link.
Цвет, hover, focus и область нажатия задаёт Base; ссылки остаются одним native
anchor, адреса и namespace/project scope сохраняются у продукта.

Scoped unit, TypeScript, lint и primitive guard прошли; AST-сверка подтверждает
прежние данные, маршруты, условия и события без подмены результата запроса.
Реальная продуктовая browser-приёмка и точный UI package/pin остаются открытыми.
Ссылки внутри текста не превращаются в полноразмерные кнопки; их общий контракт
проверяется отдельно.

## Текстовые ссылки и переход к Wiki — 2026-10-22

Две ссылки LinkEditor/TaskDocumentsPanel используют TextLink, самостоятельное
действие перехода к Wiki — Button asChild variant=link. Сохраняются route,
namespace, project_scope, query/hash и условия доступности. Прикладная логика
и события совпадают по AST; шесть scoped unit cases прошли.

TypeScript, scoped lint и primitive guard прошли. Общие компоненты проверены
в IAB на собственных синтетических сценах; фактическая продуктовая browser-
приёмка и поставка точного UI archive/pin остаются частью итогового gate.

## Опасное действие проекта — 2026-10-23

Пункт удаления ProjectCardActionItems использует DropdownMenuItem tone="danger"
Base без локального text-danger. Условия canAdmin, namespace/space context и
переданный onDelete сохранены. Пять scoped unit cases PASS: действия привязаны
к точному проекту, user не получает удаление, открытие меню не вызывает callback,
Enter на разрешённом пункте вызывает его один раз. TypeScript и scoped ESLint
прошли. Итоговая поставка UI и IAB всего продуктового экрана остаются в общем gate.


## Общая граница query-сессии — 2026-10-23

Base предоставляет PlatformQuerySession и platformQueryDefaults через
`@pdlc/ui/platform`. Wiki/Fleet передают стабильный адаптер authVersion/subscribe,
фабрику своего QueryClient и QueryClientProvider с прежним SpaceAdmissionBoundary.
Store, права, expiry, SSO и запросы остаются у продукта. Смена подтверждённой
версии очищает старый cache и перемонтирует формы; pending sign-out не повышает
версию сам по себе. Tracker использует только общий retry/backoff: его lifecycle
SpaceSessionBoundary сохраняется отдельно и этим переносом не меняется.

Настройки чтения сведены без изменения исполняемой политики: transport/429/5xx,
не более четырёх повторов, backoff 1/2/4/8/10 секунд; mutations retry=false.
Base не получает зависимости от React Query. Адресная AST/JS-сверка трёх прежних
политик и пяти публичных platform-функций прошла; 9 scoped Base tests, compilation
самостоятельных examples и TypeScript/Lint трёх потребителей PASS.

Consumer auth-boundary unit gate пока не подтверждён: требуется изолированное
frozen окружение вместо прежнего смешанного React graph. IAB проверяет только
синтетическую композицию Base без token/SSO/backend: Enter даёт одну редакцию,
старый черновик удаляется, внешний trigger сохраняет focus. 320/1920 без overflow,
действие 44/40 px. Это source-кандидат, не обновление UI pin и не runtime-приёмка.
