# Frontend Standards — Task Tracker

> Стартовый документ. До конца разработки часть соглашений может измениться — актуализировать при стабилизации frontend-реализации.

## 1. Scope

Соглашения для frontend-подсистемы Task Tracker: архитектурные слои, именование, работа с API, формы, состояние, стилизация, тестирование и производительность.

## 2. Архитектурные слои (Feature-Sliced Design)

Проект использует FSD-методологию. Каждый слой имеет чёткие права импорта:

| Слой | Ответственность | Может импортировать из |
|---|---|---|
| `app` | Инициализация, роутер, providers, глобальные стили | `pages`, `widgets`, `features`, `entities`, `shared` |
| `pages` | Страницы, композиция виджетов | `widgets`, `features`, `entities`, `shared` |
| `widgets` | Самостоятельные блоки интерфейса | `features`, `entities`, `shared` |
| `features` | Пользовательские сценарии | `entities`, `shared` |
| `entities` | Бизнес-сущности | `shared` |
| `shared` | Переиспользуемые примитивы, API-клиент, i18n, utils | — |

## 3. Именование и структура сегментов

Каждый слайс состоит из сегментов:

- `ui/` — компоненты.
- `model/` — бизнес-логика, stores, selectors.
- `api/` — запросы к backend.
- `lib/` — вспомогательные функции.
- `config/` — конфигурация слайса.

Пример:

```text
features/
└── issue-create/
    ├── api/
    │   └── create-issue.ts
    ├── model/
    │   ├── store.ts
    │   └── types.ts
    ├── ui/
    │   └── IssueCreateForm.tsx
    └── index.ts
```

Публичный API слайса экспортируется только через `index.ts`.

## 4. Компоненты

- Использовать функциональные компоненты + hooks.
- Презентационные компоненты не должны знать о состоянии приложения.
- Container-компоненты подключают store и передают данные вниз.
- Props-интерфейсы именуются `{ComponentName}Props`.
- Составные компоненты (compound) оформляются как единый объект с подкомпонентами.

## 5. Работа с API

- Все API-запросы проходят через `@tanstack/react-query`.
- Ключи запросов типизированы и централизованы в `shared/api/query-keys.ts`.
- Мутации сопровождаются инвалидацией связанных query-ключей.
- Ошибки API обрабатываются глобальным обработчиком и отображаются через `sonner`.
- Retry-политика: 3 попытки при 5xx/NetworkError, 0 попыток при 4xx.

## 6. Формы

- `react-hook-form` + `zod` для валидации.
- Схемы валидации хранятся рядом с формой в `features/<name>/lib/schema.ts`.
- Каждая форма должна поддерживать состояния: pristine, submitting, submit error, success.
- Disabled состояние всех полей при `isSubmitting`.

## 7. Состояние

- Серверное состояние — `@tanstack/react-query`.
- Глобальное клиентское состояние — `zustand` (максимум 3-4 stores).
- Локальное состояние компонента — `useState`/`useReducer`.
- Side-effects внутри features — через `effector` или кастомные hooks, если усложняется логика.

## 8. Стилизация

- Tailwind CSS 4.1.0 + `@tailwindcss/vite`.
- Кастомные классы и design tokens — через `frontend/src/styles/index.css`.
- Использовать `cn()` из `shared/lib/cn.ts` для условных классов.
- Цвета, отступы, типографика — только из design tokens.
- Адаптивность: mobile-first, breakpoints `sm`, `md`, `lg`, `xl`, `2xl`.

## 9. Маршрутизация

- `react-router` 8.1.0.
- Страницы регистрируются в `app/router.tsx`.
- Lazy loading для всех страниц кроме login и dashboard.
- Layout-компоненты в `app/layouts/`.

## 10. Тестирование

- Unit/интеграционные — Vitest + React Testing Library.
- E2E — Playwright.
- Каждый feature покрывается через user-centric сценарии.
- После UI-изменений — full-page скриншоты 375 / 1920 / 2560.

## 11. Производительность

- React Compiler и `memo` только после профилирования.
- Виртуализация длинных списков — `@tanstack/react-virtual`.
- Code-splitting по страницам и крупным виджетам.
- Ассеты кэшируются через Vite PWA/service worker (при реализации).

## 12. i18n

- Локали: `ru` (default), `en`.
- Ключи именуются в kebab-case: `issue.create.title`.
- Тексты не хардкодятся в компонентах.

## 13. References

- `docs/FRONTEND_ARCHITECTURE.md` — технический стек и структура.
- `docs/DESIGN_TOKENS.md` — токены дизайна.
- `docs/REACT_STYLING.md` — руководство по стилизации.
- `docs/UI_LIBRARIES.md` — выбор библиотек.
- `docs/TESTING.md` — стратегия тестирования.
- `docs/CODE_STYLE.md` — общий code style.
- `docs/API.md` — REST API.
- `docs/ERROR_HANDLING.md` — обработка ошибок.
