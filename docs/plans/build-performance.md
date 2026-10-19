# Быстрые повторные проверки и сборка

Ordinary backend CI после workspace build использует
`${CARGO_TARGET_DIR:-target}/debug/gen-openapi`. Прежний output path и schema diff
сохранены; generator не пересобирается через отдельный Cargo graph.

Применить [стандарт Base](https://github.com/FerrPOINT/services-base/blob/197ead9d9b6cf6901e8f9dd31f43f719885502c3/docs/BUILD_PERFORMANCE.md) без изменения runtime и SDK/UI/skills pins.
Product-owned команды описаны в .quality-gate.json. При разработке/сборке —
unit/static; integration/slow/full coverage — вручную перед релизом.
Security/data/compatibility/live gates сохранены; reuse только для допустимых
deterministic exact-candidate проверок.
Проверить no-change, один компонент, dependency/toolchain change и corrupt output;
измерить duration и cache hit/miss. Сохранять прогретые caches и чистить own scratch.

Docker frontend: раздельные unit/compile stages, unit/static перед compilation;
TypeScript проверяется один раз в обязательном unit stage; compile запускает Vite.
production COPY исключает тесты. Frozen dependencies сохраняют общий совместимый
pnpm download store. Parent digests закреплены; Dockerfile frontend, общий RUN slot и
ограничения Windows producers определяет стандарт Base.
Runtime и package build scripts сохраняют прежние публичные команды.

Frontend CI handoff: один `dist` с manifest/digests, затем verified nginx packaging.
Команда self-check и product flags: [TESTING](../TESTING.md#передача-frontend-artifact). SDK/UI/skills pins сохраняются.

Отдельные test setup и `vitest.config.ts` исключены из production COPY
и `frontend:build` inputs, но остаются входами unit/static. Только эти явно
объявленные пути допускают пропуск CI compilation; общий `vite.config.ts`
с aliases/plugins остаётся полноценным build input. Ошибка unit блокирует
получение нового образа даже при сохранённой production compilation.

Docker installation получает manifest без `scripts` через отдельный cheap
BuildKit stage и content-addressed COPY. Установка выполняется с
`--frozen-lockfile --ignore-scripts`, сохраняет lock/workspace и настоящий Base
package. Полный product manifest возвращается перед проверками и compilation.
OpenAPI postinstall выполняется один раз в отдельном generated stage; spec,
generator и полный manifest определяют его cache key. Непустые outputs
проверяются и копируются после frontend sources в unit/application stages.
Изменение scripts/spec не запускает повторный install; отсутствие output или
ошибка generation блокируют результат, включая прогретый compile cache.

CI Rust target после restore готовится через `python3 ../scripts/vendor/base_rust_target_cache.py` из `backend/`; версия и checksum включены в отдельный Base quality packet.

## 2026-10-19: test-only inputs для всех 59 domain units

Четыре terminal inline модуля `jql::tests` (15 cases),
`sdlc_execution_lease::tests` (1), `sdlc_native_admission::tests` (4) и
`sdlc_pm_draft::tests` (3) перенесены в отдельные `src/tests/*.rs` через
`#[cfg(test)]` и `#[path]`. Имена test paths, helpers/imports, assertions и private
access сохранены; production bytes до прежних модулей не меняются. Остальные
36 cases остаются в прежних внешних test modules; всего domain inventory — 59.

Оба backend Dockerfile сохраняют полный unit COPY, но production COPY исключает
ровно девять test-only файлов. Четыре прежних пути `entities/tests.rs`,
`repositories/tests.rs`, `stubs/memory/tests.rs`, `value_objects/tests.rs` явно
включены в artifact exclusions и `backend_unit_inputs`: directory pattern
`**/tests/**` сам по себе не покрывает basename `tests.rs`. Новые четыре файла и
прежний `tests/project_directory.rs` покрыты существующим directory pattern.

Scoped pure source controls проверяют inventory, cfg declarations, обе COPY
границы, artifact input projection, selector и CI classification для всех девяти
файлов и production neighbors. Source checks не заменяют native qualification.
Existing CI module доступен отдельно как `tooling:build-contracts` в unit/build
profiles. Изменение только этого script не выбирает frontend tests или Cargo;
ordinary CI по-прежнему вызывает существующий module один раз.

Одна integrated native веха нового tuple остаётся PENDING: полные 59 domain units,
normal production build без test-only файлов, unit-only edit с older mtime при
сохранении dependency/application layers и ELF, failing-unit rejection,
production-neighbor invalidation и runtime payload. SDK/UI/tooling pins, manual
integration/release gates, caches и общий host slot сохраняются. Прежние native
proofs относятся к прежним source/recipe tuples; CPU/RAM/time savings не заявлены.
