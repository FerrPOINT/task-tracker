# Стадии стандартной доски — 2026-11-02

Основание: Base `docs/platform/SDLC.md` и Tracker `SDLC_LIFECYCLE_V1.md`.
Девять стадий независимы от Ready/InProgress/Completed. Обычная доска раньше
группировала по четырём status UUID и скрывала SDLC-задачу Backlog/Ready.

Теперь новые стандартные доски используют автоматическую проекцию стадий.
Точный неизменённый старый шаблон адаптируется при чтении; пользовательские
status-доски сохраняются. Стадии берутся одним запросом к состоянию SDLC,
legacy Clarification относится к Draft. Обычным задачам стадия не выдумывается.
Статус показан на карточке; общие move-команды не предлагаются для SDLC.
Узкий экран выбирает первую заполненную стадию при отсутствии выбора в URL.

Бэклог считает и выбирает несвязанные со спринтом задачи стадии Backlog,
независимо от их операционного status UUID. Список спринта принадлежит только
действительно активному спринту. Имена статусов берутся из каталога.
Новых миграций и перезаписи задач/назначений/истории нет. API и OpenAPI обновлены.

## Проверки исходников

- 855 обычных Rust-тестов по workspace: API/App — 628, остальные пакеты — 227.
  Результаты API/App переиспользованы при неизменном production-коде. Старое
  ожидание трёх default-колонок обновлено; последний Domain-прогон проходит.
- Реальный PostgreSQL/HTTP сценарий создания, согласования и завершения PM:
  задача Backlog/Ready найдена и в колонке Backlog, и в backlog_issues;
  девять стадий, независимый статус, отсутствие phantom sprint — PASS.
- Clippy workspace/all-targets/no-deps с `-D warnings`, rustfmt — PASS.
  После PostgreSQL-проверки менялось только эквивалентное форматирование двух
  URL в тесте; production-код и запросы не изменились.
- 857 frontend unit tests; финальные 18 целевых тестов доски/бэклога — PASS.
- TypeScript/Vite build, полный ESLint/semantic/shell, OpenAPI generation/check,
  целостность принятого UI pin — PASS.
- Chromium/Firefox/WebKit, 375/1920/2560 px: 9 сценариев на финальном UI без
  автоматических retries. Проверены заполненная стадия, независимый статус,
  выбор стадии в URL, переход в бэклог, возврат на доску и отсутствие overflow.
  API fixtures, бизнес-записей и ошибок страницы — 0. При нехватке памяти
  браузерный прогон выполнен последовательно с лимитом 60 s.

[Снимки и размеры PNG](../assets/screens/sdlc-board-stages-20261102/manifest.json).
Full-page captures сохранены без редактирования. WebKit использует native DPR2;
ширина viewport 2560 соответствует PNG 5120 px.

## Приёмка установленной версии

Normal Build/Apply в sdlc3 завершены 2026-11-02. Исходники приложения:
`136ba054f128ee8e62f5ef1e5ffe11b5f59c6019`, build:
`9eba4cb8b2704f9e992eee365e6c40e1`. Образы и проверки сохранности:
[installed-runtime.json](../assets/screens/sdlc-board-stages-20261102/installed-runtime.json).

API установленного Tracker возвращает девять стадий. SDLC3-1 видна в Backlog,
операционный статус Ready, подтверждённая редакция требований 2. Бэклог содержит
одну задачу; при отсутствии спринта sprint_issues пуст. Контекст задачи и образы
Fleet сохранены. В браузере проверены доска, карточка и переход в бэклог/обратно.
[Снимок установленной доски](../assets/screens/sdlc-board-stages-20261102/live-1920.png).

Для сборки применён host operator Base `f666e82a7101d8ee472ce180062f4eabf941a33d`:
[consumer UI binding](https://github.com/FerrPOINT/services-base/pull/440).
Публикация Base согласована с актуальным main; 44 scoped unit tests — PASS.
SDK/UI pins продукта не менялись. Kubernetes rollout не проверялся.

Глобальный Docker groups audit имеет ранее существовавшие нарушения в соседних
ресурсах; собственных board-stages QA-контейнеров после cleanup нет.

## Счётчики в компактном переключателе

Подписи стадий включают число видимых задач: «Бэклог (1)», «Анализ (0)».
Используется тот же список, что и в колонке, без дополнительных API-запросов.
Целевые unit-проверки (9), TypeScript/Vite и ESLint — PASS. Chromium на
375/1920/2560 px: 3 сценария — PASS, включая все девять подписей на телефоне,
пустую/заполненную стадии, URL selection и отсутствие overflow.
[Полноразмерные снимки](../assets/screens/sdlc-stage-counts-20261102/manifest.json).

Полный frontend-прогон после правки: 88 файлов / 857 тестов — PASS.
Normal web-only Build/Apply установлен в sdlc3 из `90891bd3`;
[образ и preservation checks](../assets/screens/sdlc-stage-counts-20261102/installed-runtime.json).
В живом браузере на 539 px видны «Бэклог (1)» и восемь пустых стадий с нулями.
[Снимок](../assets/screens/sdlc-stage-counts-20261102/live-539.png).
Tracker backend, Fleet images и контекст задачи сохранены.
