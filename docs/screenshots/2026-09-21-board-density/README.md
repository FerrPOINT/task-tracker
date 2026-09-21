# Board density browser QA

Дата проверки: 2026-09-21.

## Объём

- Production preview актуального frontend bundle.
- Изолированная доска: 4 колонки и 24 задачи через публичный API-контракт.
- Вход через настоящий SSO callback flow с тестовым ES256 ID token и JWKS.
- Viewport: 375x812, 768x900, 1440x900, 1920x900 и 2560x1200.
- Темы: light и dark; на tablet/desktop проверены обычная и компактная плотность.
- Всего: 18 browser-состояний.

## Результат

- Horizontal overflow, console/page/HTTP errors: 0.
- Serious/critical axe findings: 0.
- Маленькие и безымянные кнопки в рабочей области: 0.
- Переключатель скрыт на mobile, доступен с 768 px, меняет `density` в URL и
  сохраняет active state; восстановление из URL закреплено unit-тестом.
- Mobile остаётся компактным по умолчанию и показывает одну выбранную колонку.
- Средняя высота карточки в compact-режиме: 102 px вместо 119 на 1440 px,
  92 вместо 109 на 1920 px и 82 вместо 94 на 2560 px.
- Metadata, аватар, приоритет, тип задачи, DnD и 40 px status action сохранены.

`qa-results.json` содержит машинные метрики всех состояний. PNG — full-page
снимки обычной и компактной плотности; проверка изолированная и не подтверждает
live backend mutations или production Central Auth.
