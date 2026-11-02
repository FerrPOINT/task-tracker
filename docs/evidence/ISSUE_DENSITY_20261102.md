# Полное описание и компактная карточка задачи

Описание показывается целиком на всех размерах экрана, без копии-превью и
кнопки раскрытия. Для пустых метаданных, активности, комментариев и журнала
используются короткие строки внутри существующих панелей. Стандартный
CardHeader/CardContent density=compact уменьшает внутренние отступы.
Все действия, данные, проверки прав и ошибки загрузки сохраняются.

Проверки: 19 файлов / 144 unit tests затронутых областей — PASS;
TypeScript/Vite build и scoped ESLint — PASS. Chromium на 375/1920/2560 px:
3 сценария — PASS; проверены конец длинного описания, отсутствие раскрытия,
пустые состояния после успешных чтений и отсутствие overflow. API fixtures,
бизнес-записей и page errors — 0. Снимки просмотрены:
[fixture manifest](../assets/screens/issue-density-20261102/fixture-manifest.json).

Normal web-only Build из `9b6c7f36` успешно собрал образ, build
`b7f9269f89fb4775bf18ee71d8419a68`. Normal web-only Apply выполнен; Tracker backend, Fleet и контекст задачи
сохранены. Установленная версия проверена в браузере на 375/539/1920/2560 px:
полный текст, компактные пустые состояния и отсутствие overflow.
[Runtime manifest](../assets/screens/issue-density-20261102/installed-runtime.json),
[геометрия](../assets/screens/issue-density-20261102/geometry.json),
[карточка на 539 px](../assets/screens/issue-density-20261102/full-539.jpg).

На той же задаче высота страницы при viewport 539 px уменьшилась с 2085 до
1613 px, при 375 px — с 2203 до 1755 px. Native screenshots сохранены без
перекодирования; JPEG имеет ширину content viewport без полосы прокрутки.
UI/SDK pins, API и данные не менялись.
