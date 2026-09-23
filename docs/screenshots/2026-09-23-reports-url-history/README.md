# Reports: browser history, canonical URL и responsive tables

Проверка выполнена 23.09.2026 в Chromium на production-сборке ветки с
настоящими Central Auth, shell и Task Tracker API. Компонентная матрица была
read-only; изменяющие сценарии выполнялись отдельным live regression с cleanup.

## Воспроизведение

До исправления выбор проекта, вкладки и спринта всегда заменял текущую запись
history, поэтому Back/Forward не восстанавливали предыдущий отчёт. Default
`tab=velocity` и неизвестный `tab` оставались в URL. Production QA также
подтвердил horizontal overflow cumulative flow до 543 px на viewport 375 px и
до 791 px на viewport 768 px: длинные ISO-даты расширяли `<table class=sr-only>`.

## Результат

- Project, tab и sprint создают отдельные записи history и сохраняют соседние
  query-параметры.
- Direct URL, reload, Back и Forward восстанавливают выбранный отчёт.
- `velocity` остаётся canonical URL без `tab`; неизвестный `tab` удаляется
  через replace.
- Повторные Radix Tabs events не создают дублированные записи history.
- Основные controls имеют высоту 44 px на mobile и 40 px на больших экранах.
- Доступные таблицы графиков находятся в clipping-wrapper; cumulative flow с
  длинными датами больше не расширяет document на mobile и tablet.

Матрица: 4 вкладки × 5 viewport (`375`, `768`, `1280`, `1920`, `2560`) × 3 темы
(`light`, `gray`, `dark`) = 60 состояний. Во всех состояниях нет horizontal
overflow, малых целей внутри `main`, безымянных controls, неожиданных вложенных
скроллеров, серьёзных/критических Axe-нарушений и runtime/HTTP 5xx ошибок.

Дополнительно на той же сборке прошли:

- `task-pages-live.spec.ts`: 1/1 полный route matrix Task Tracker;
- `functional-live.spec.ts`: 7/7 живых сценариев платформы с cleanup.

Артефакты:

- `qa-report.json` — URL-flow и машинно-читаемые результаты 60 состояний;
- `velocity-375-light.png` — mobile default report;
- `burndown-1920-gray.png` — desktop Burndown;
- `control-chart-2560-dark.png` — wide desktop control chart.
