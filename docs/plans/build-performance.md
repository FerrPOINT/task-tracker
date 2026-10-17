# Быстрые повторные проверки и сборка

Ordinary backend CI после workspace build использует
`${CARGO_TARGET_DIR:-target}/debug/gen-openapi`. Прежний output path и schema diff
сохранены; generator не пересобирается через отдельный Cargo graph.

Применить [стандарт Base](https://github.com/FerrPOINT/services-base/blob/7d98497e8ce5f5206f00b9e33b9a5c504ce02f96/docs/BUILD_PERFORMANCE.md) без изменения runtime и SDK/UI/skills pins.
Product-owned команды описаны в .quality-gate.json. При разработке/сборке —
unit/static; integration/slow/full coverage — вручную перед релизом.

В PDLC2 modular SDK/UI paths выбираются из product manifests при прежних exact
pins. Production domain source сохраняется; directory tests вынесены без
изменения содержания в `backend/domain/src/tests/project_directory.rs`, чтобы
их изменение не инвалидировало runtime compilation. Backend image сохраняет
только прежний `server`; полный workspace/CLI inventory остаётся в release.

Обычный CI не поднимает PostgreSQL ради domain units. Ручной
`workflow_dispatch` с `http_only=true` сохраняет targeted HTTP clippy,
disposable PostgreSQL drafts/sdlc suites и OpenAPI drift. Полный `release.yml`
дополнительно сохраняет populated0033→0096 upgrade, strict EOL/exact test count,
restore, namespace projection и MCP upgrade gates. Source shape не является
новым native/integration/release PASS.

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
