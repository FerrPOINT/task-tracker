# ADR-0001: Rust + Axum for Backend

## Status

Accepted

## Context

Нужен современный backend-стек для self-hosted таск-трекера с Spring-подобной архитектурой: DI, слои, готовые библиотеки, производительность и безопасность.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| Go + Gin/Echo | Простота, быстрая сборка | Менее выразительная тип-система, ручной DI |
| Java + Spring Boot | Зрелый DI, библиотеки | Тяжеловат для self-hosted, JVM memory |
| Node.js + NestJS | TypeScript end-to-end | single-thread, производительность |
| Rust + Axum | Безопасность, скорость, async, DI через traits | Learning curve, compile time |

## Decision

Использовать Rust 1.88+ с Axum 0.8.9.

## Consequences

- Сильная типобезопасность на всех уровнях.
- Высокая производительность при низком потреблении ресурсов.
- Ручная настройка DI через `AppContext` / `shaku`.
- Нужно больше времени на onboarding разработчиков без опыта Rust.

## Related

- `docs/ARCHITECTURE.md`
- `docs/LIBRARIES.md`
