# Admin: URL-состояние вкладок и размеры целей

Проверка выполнена 23.09.2026 в Chromium на production-сборке ветки с
настоящими Central Auth, shell и Task Tracker API. Компонентная матрица была
read-only; изменяющие сценарии выполнялись отдельным live regression с cleanup.

## Воспроизведение

До исправления выбор `Журнал аудита` хранился только в React state: URL не
менялся, reload и прямое открытие возвращали `Настройки инстанса`, а Back/Forward
не восстанавливали вкладку. На desktop вкладки, поле ключа, сохранение настройки
и загрузка следующего окна аудита могли иметь высоту 32–36 px.

## Результат

- `tab=audit` сохраняет журнал аудита в URL; default-вкладка не добавляет
  лишний параметр.
- Прямой URL, reload, Back и Forward восстанавливают выбранную вкладку без
  дублированных записей history.
- Неизвестный `tab` и `tab=settings` канонизируются через replace, соседний
  `source=qa` сохраняется.
- Вкладки и основные controls имеют высоту 44 px на mobile и 40 px на больших
  экранах.

Матрица: 2 вкладки × 5 viewport (`375`, `768`, `1280`, `1920`, `2560`) × 3 темы
(`light`, `gray`, `dark`) = 30 состояний. Во всех состояниях нет horizontal
overflow, малых целей внутри `main`, безымянных controls, неожиданных вложенных
скроллеров, серьёзных/критических Axe-нарушений и runtime/HTTP 5xx ошибок.

Дополнительно на той же сборке прошли:

- `task-pages-live.spec.ts`: 1/1 полный route matrix Task Tracker;
- `functional-live.spec.ts`: 7/7 живых сценариев платформы с cleanup.

Артефакты:

- `qa-report.json` — URL-flow и машинно-читаемые результаты 30 состояний;
- `settings-375-light.png` — mobile settings;
- `audit-1920-gray.png` — desktop audit;
- `audit-2560-dark.png` — wide desktop audit.
