# Task Tracker

Self-hosted таск-трекер, аналог Jira. Backend — Rust, frontend — React.

## Структура

- `backend/` — Rust workspace (Axum + SQLx + PostgreSQL)
- `frontend/` — React 19 + Vite 6
- `cli/` — Rust CLI для администрирования
- `docs/` — архитектура, ТЗ, производительность, тестирование

## Документы

- [Архитектура](docs/ARCHITECTURE.md)
- [Техническое задание](docs/TZ.md)
- [Производительность](docs/PERFORMANCE.md)
- [Тестирование](docs/TESTING.md)

## Стек (актуальные версии)

- Rust 1.88, Axum 0.8.9, Tokio 1.52.3, SQLx 0.9.0
- React 19.1, Vite 6.2, TypeScript 5.9.3, Tailwind 4.1, shadcn/ui
- PostgreSQL 17, Redis 8
- Vitest 4.1.10, Playwright 1.51.1, testcontainers 0.27.3

## Быстрый старт

Пока в разработке.
