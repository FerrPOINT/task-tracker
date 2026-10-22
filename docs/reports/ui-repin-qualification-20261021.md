# Проверка UI-кандидата Tracker — 2026-10-21

Кандидат сохраняет merged main `9adc832b` и независимо закрепляет UI на
`94272dc5512be58ec6c88fb7c1d1d33c7599a3e4`. SDK, backend, API и миграции
совпадают с main. Расширенный OpenAPI из PR181 сохранён; генерация клиента
выполнена повторно без tracked drift.

## Исправления совместимости

- Удалён прежний `md:-ml-1.5` у логотипа Tracker. Он сдвигал знак на 6 px:
  header padding 16 px, знак 40 px, compact rail 72 px; отрицательный margin
  давал центр 30 вместо 36 px. Геометрией снова управляет Base. Browser assertion
  с допуском 1 px сохранён и проходит; новых продуктовых CSS overrides нет.
- В тесте LogWorkDialog выбран `Close`, соответствующий английской локали.
  Проверки запрета Escape/закрытия во время pending сохранены.
- В тестах API/idempotency/catalogue длинные значения вставляются обычным
  `user.paste`, без увеличения timeout. Все проверки исходного payload,
  operation ID, исправленного ввода и authoritative catalogue сохранены.
- Browser selectors используют текущие названия сворачивания/разворачивания
  меню Base. Предусловие standalone fixture каталога записано в TESTING.

## Проверки

Первый полный прогон дал 517 PASS / 6 FAIL из 523. Три сценария упёрлись
в timeout 5 секунд; незавершённый посимвольный ввод загрязнил два последующих
сценария. Отдельный diagnostic unknown-outcome до исправления завершился
успешно за 8.1 секунды с временным CLI timeout 20 секунд, сохранив все
payload/idempotency assertions. В итоговом коде timeout не увеличен.

На объединённом исходнике повторены полностью:

- issue-create, LogWorkDialog и SdlcPanel: 79/79 PASS;
- AppShell и Namespace: 29/29 PASS;
- ESLint, semantic/shell checks, Prettier изменённых frontend-файлов: PASS;
- OpenAPI drift/compatibility и actual packed UI consumer: PASS;
- обычный TypeScript/Vite production build без catalog override: PASS;
- отдельный fixture build с `VITE_PLATFORM_SERVICES_URL=/api/v1/runtime/services`: PASS;
- effective theme contrast: PASS;
- оба Chromium smoke — new task entry и login/navigation/shell: 2/2 PASS,
  без retries/skips. Они проверяют 375/1920/2560 px и дополнительные layout
  режимы; full-page screenshots сохранены в qualification evidence.

Остальные неизменённые unit suites используют результат первого полного
прогона. Это совокупная проверка затронутых сценариев, а не заявление о новом
едином полном зелёном прогоне 523 тестов.

Fixture API, каталог и SSO синтетические. Этот результат не доказывает live
backend/SSO/PM admission и не меняет установленный runtime. Собственный
временный preview остановлен; Base source и runtime volumes не менялись.
