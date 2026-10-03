# Проверка CLI Task Tracker

## Принятая поставка sdlc1 — 2026-10-03

Backend принят: `sha256:7dd8fcbd39759e230ff14e760aefadfe327dd4a413b0a800cfbde9e5871498b4`. Проверенный code source
`10eca7fbb47d356b22946e7e1360f2d9f0b4ef80`, опубликованный Base pin
`9408802dfa978cba2f67162a49adca6f65851b01`, Rust 1.88.0, locked dependencies.
Изменения main после этого source, включённые в документационный PR, не меняют
backend/CLI tree. SHA документационного merge проверяется отдельно от build source.
Предыдущий baseline image: `sha256:05ae216ded4a6992b167c97e7f54d1d0bb0078a5271e4f7332e989bcc6bdff8b`.

UUID и ключ одной задачи, read/restore по ключу с проверкой доступа, поиск/страницы, update/unassign, stdin/comments, links, attachments/download/no-clobber, заранее определённый custom field и изменение/очистка value, worklogs, transitions и trash/restore.

Установка и checksums: [CLI_INSTALL.md](CLI_INSTALL.md).
Все более ранние dated sections ниже — исторические проверки и blockers,
снятые этой поставкой, если явно не указано сохранённое ограничение.

### Сохранность, откат и границы

По отдельному решению пользователя принят новый чистый sdlc1, подготовленный
другой задачей после потери прежнего Docker/data. Утраченные старые данные и
images не восстановлены этой поставкой. Baseline отката — точные images нового
чистого стенда, сохранённые до обновления; восстановление утраченного старого
стенда не заявляется.

Свежий согласованный backup трёх БД и пяти фактических файловых volumes:
`20261003T142547-890aba`, 11 payloads с проверенными checksums. Перед ним
проверены нулевые active jobs/queue/leases; остановлены только три писателя.
Отдельная fresh-copy QA использовала backup `20261003T122738-8a92db`: restore
45/31/17 таблиц CI/CD/Task/Wiki, rows/sequences/schema/constraints/owners/grants
и hashes/ownership пяти volumes совпали. Эти volumes были пустыми на новом
baseline; сохранность файлов дополнительно подтверждена fixture upload/Git/
artifact/download сценариями, а не восстановлением потерянных старых файлов.

На копии запуск кандидатов, затем точных прежних images не изменил исходные
данные и схему. Старые images проходят 114 проверок и сохраняют четыре известные
ограничения: Task read/restore по ключу, Wiki legacy multipart replay и старое
CI/CD ограничение generic deployment только для Pulse. Поэтому откат возвращает
прежнее поведение, а не гарантирует исправленные CLI-сценарии. Неожиданных
отказов финальной репетиции нет. Первый rollout откатился из-за преждевременной
проверки Docker health `starting`; после исправления локального readiness wait
второй rollout принят. Рабочие данные не восстанавливались и ledger не правился.

Под workspace lock атомарно заменены только три image pins, затем последовательно
Task → Wiki → CI/CD через Compose `up --no-deps --no-build --pull never`.
Readiness: HTTP и Docker healthy, deadline 180 секунд на сервис. Откат: под тем же
lock вернуть три сохранённых pins и пересоздать эти сервисы в том же порядке;
не выполнять автоматический restore рабочих данных. Защищённые backups,
runtime-before и rollout manifest сохранены локально; секреты не публикуются.

Наблюдение: 900 секунд, 31 sample каждые 30 секунд, readiness 200/healthy,
0 restarts, 0 новых ERROR в обоих потоках логов. Остальные контейнеры/mounts
не менялись нашим rollout. Во время наблюдения отдельная задача обновила
admin-api/admin-web/ai-runtime sdlc2; их images сверены с её build/apply receipt,
mounts сохранены. Это отражено отдельно, глобальная неизменность sdlc2 за весь
интервал не заявляется. Наши keys/runtime fingerprints остались неизменными.

QA-кандидаты: 123 основных + 10 дополнительных проверок — PASS. Live: 123
успешные проверки и 6 focused checks — PASS. Отдельная ранняя попытка template
`type=page` получила корректный validation 400: ошибка fixture, исправлена на
поддерживаемый `release_note`; успешный повтор записан отдельно, исходный отказ
не скрыт. JSON/204, confirmations, stdin, access/validation/conflict, redaction,
transport timeout, no-clobber и cleanup проверены регрессиями и CLI-приёмкой.
Execution states задавались собственными fixtures/API; production runner и
внешний deploy не сертифицируются. Pulse — обычный тестовый repository/PR/pipeline
в CI/CD, отдельного постоянного стенда нет. Собственные PAT отозваны, QA Compose
ресурсы и временные keys удалены; принятые и прежние backend images сохранены.

## Исторические проверки

Проверено 2026-10-02 в изолированном task checkout `feat/cli-workflows`.

## Среда и обязательные gates

Ubuntu WSL, rustc 1.88.0 (6b00bc388 2025-06-23), Node 22.20.0 / pnpm 10.28.1, Python 3.12.3. Проверено после объединения с актуальным продуктовым `main`, с чистым опубликованным Services Base `c083783a37791e277db796361203884b87828a7d`, закреплённым в `.base-revision`. Принятая в `main` политика pinned Base, Cargo `--locked` и pnpm `--frozen-lockfile` сохранена. Base и исходные dirty checkout этой задачей не изменены.

Backend: fmt, workspace/all-target Clippy с `-D warnings`, workspace tests, отдельный MSRV check (`cargo check --locked --workspace --all-targets`), OpenAPI drift и release workspace — успешно. Workspace: **511 passed, 21 ignored**, 0 failed. Workspace выполнен последовательно, как предусмотрено workflow этого продукта.

```bash
cd backend
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
cargo build --locked --release --workspace
TT_TEST_DATABASE_URL=postgres://.../tasktracker_infra_test cargo test --locked -p infra --test repos deleted_issue_key_can_be_resolved_for_restore_only -- --ignored --test-threads=1
```

Docs validators и существующие CI-contract tests проходят. Frontend: install с `--frozen-lockfile`, OpenAPI check/compat с `origin/main`, tests, lint и build — успешно. Дополнительно проходят неизменность generated contracts/lockfile, packed Base consumer и effective themes (dark/gray/light) на built preview. Task Tracker дополнительно typecheck; Task Tracker/Wiki — предусмотренный format check. Frontend tests этого продукта: 257.

## Регрессии и проверенные сценарии

- Новый subprocess regression проверяет восемь операций в table/compact/JSON, HTTP 200 с пустым body и 204, а также access denial без ложного success: 72 сценария. До исправления терялось прежнее текстовое подтверждение; после него тексты восстановлены, JSON остаётся `{"status":"ok"}`.
- Real API CLI lifecycle: UUID/key, statuses/transitions, stdin, update/unassign, links, custom fields/worklogs, attachments, delete/trash/restore; production handlers/services с memory repositories.
- Дополнительно выполнен ignored PostgreSQL test `deleted_issue_key_can_be_resolved_for_restore_only` на `postgres:17.6-alpine`, БД строго `tasktracker_infra_test`. Остальные ignored persistence tests не заявляются как выполненные.

Существующие проверки file/stdin, pages, JSON/204, access/validation/conflict, transport timeout, credential redaction и download no-clobber сохраняются и проходят. Новые regressions воспроизвели замечания на исходной ветке, затем прошли после исправлений.

## Границы подтверждения

Проверки используют только fixture данные и собственные временные ресурсы. Постоянные Compose-группы, runtime images, volumes и production deployment не менялись. Windows native linking недоступен (`link.exe`); Rust gates выполнены в WSL. Новых endpoint, миграций или изменений Services Base нет. Эти проверки были выполнены до merge; статус main и живая приёмка приведены ниже. Deploy не выполнялся.

Описание команд, configuration, input/output/errors и ограничения: [CLI.md](CLI.md).

## После merge и приёмка sdlc1 — 2026-10-02

Merge commit: `fd3337b200d1ec3ec6626900a9c96e2046cbed19`. [Post-merge CI](https://github.com/FerrPOINT/task-tracker/actions/runs/37000149231): docs/backend/frontend/minimum-rust — 4/4 success на этом точном SHA; содержимое merge tree совпало с reviewed head. CLI release binary собран отдельно с закреплённым Base; checksum и установка: [CLI_INSTALL.md](CLI_INSTALL.md).

На собственном проекте выполнены create/stdin, update/unassign, statuses/types/transitions, фильтры и две страницы, search, комментарии, compact confirmation, связи, upload/download/no-clobber/delete, чтение custom fields, worklogs, переход, delete/trash/restore и JSON confirmation. UUID работает, но чтение и восстановление по ключу задачи возвращают HTTP 400 VALIDATION_ERROR на текущем backend. Остальные операции после проверки этого отказа выполнялись через UUID; это не подтверждает приёмку ключей. Read-only PAT получает 403; невалидный token — 401; неверный limit — 400. Custom field set не выполнялся: fixture не создаёт определения полей, а новый проект их не содержит. Временный проект удалён через CLI.

Проверка выполнялась с временными Central Auth PAT, ограниченными тремя продуктами; read/write и read-only tokens отозваны после прогона. Значения tokens/credentials не сохранялись в логах или артефактах. Исходные dirty checkout, постоянные Compose-группы, runtime images/pins и volumes не изменялись. Fixtures использовали реальные API и PostgreSQL работающего sdlc1, но только собственные project/repository/space и файлы. Ошибки исправленного smoke (имя флага search, when: manual, начальный deployment status и вывод terminal wait) отделены от воспроизведённых отказов runtime.

**Статус:** CLI main/CI проверен; полная совместимость с текущим sdlc1 не принята. Требуется отдельная сверка/обновление backend до согласованных main-кандидатов и повтор блокирующих операций. Публичный release не объявляется готовым.

## Подготовка backend-кандидатов — 2026-10-02

Task checkout обновлён merge актуального опубликованного main без переписывания истории. Текущий продуктовый pin Base: `9408802dfa978cba2f67162a49adca6f65851b01`; он отличается от Base предыдущей CLI-поставки. Чистый Base checkout проверен через verify_base_revision. Предыдущие результаты не подменяют новую проверку этого pin.

Повторно выполнены 5 documentation regression tests и штатный documentation validator — PASS; git diff --check — PASS. Новая проверка backend/frontend, PostgreSQL fixtures, сборка образов, restore rehearsal и live acceptance не завершены: C: заполнен, Docker containers API возвращает HTTP 500, затем WSL стал возвращать E_UNEXPECTED. Попытка локального PostgreSQL старта завершилась с exit 1 без подтверждённого запуска и без выполненных cargo gates.

Runtime pins не записывались, образы не заменялись, миграции и restore не запускались, постоянные сервисы не перезапускались. Новых PAT и API fixtures не создавалось. Исправность текущего runtime и очистку временного WSL build root нужно подтвердить после восстановления окружения. Эта попытка не устранила ранее описанные live-блокеры и не подтверждает готовность новой поставки.

## Возобновлённая проверка кандидатов — 2026-10-02

Проверены опубликованный main `307e0498c5385b89a75f261916edca8e0d8aaf83` и замороженный task source `a1f67ce088bb6f2c0b1046de0c17bc1194f48c9d`; backend tree task source совпадает с main. Все продукты используют чистый опубликованный Base `9408802dfa978cba2f67162a49adca6f65851b01`. Операционные инструменты backup/restore сохранены отдельным snapshot локального Base с hashes/status: они не представлены как чистый опубликованный Base.

Rust 1.88.0: fmt, minimum Rust (locked workspace/all-targets), Clippy с `-D warnings`, workspace tests, OpenAPI drift и locked release workspace build — PASS. 511 workspace tests (21 ignored); отдельно ignored restore-key PostgreSQL test — 1/1 PASS. Node 22.20.0 / pnpm 10.28.1: frozen install, OpenAPI check/compat, frontend tests (257), lint/build, generated-contract/lockfile check, packed Base consumer и effective themes — PASS. Task Tracker typecheck и предусмотренный format check Task Tracker/Wiki — PASS. Первоначальный Wiki format failure локализован в CRLF export; canonical LF export проходит без изменения исходников.

Собран продуктовый Dockerfile с Rust 1.88.0 и locked dependencies из canonical LF source; image ID `sha256:7dd8fcbd39759e230ff14e760aefadfe327dd4a413b0a800cfbde9e5871498b4`. Branding builder не применялся. Штатный пользователь, бинарник, runtime libraries, migrations (где поставляются файлами) и доступность собственного uploads/storage каталога проверены. Runtime pins не изменялись.

UUID и ключ дают одну задачу; restore по ключу, trash, переходы, поиск/пагинация, unassign, комментарии/stdin, связи, вложения/download/no-clobber, worklogs и JSON confirmations проходят на образе с восстановленной PostgreSQL-копией. Отдельная QA fixture заранее определила custom field через API: CLI set из stdin, read и clear значений — PASS.

Общий изолированный CLI-прогон: 123/123 PASS; дополнительный прогон custom-field values и Wiki partial recovery: 10/10 PASS, включая повторные auth checks. Использованы временные QA identity/PAT и отдельный native Auth fixture из чистого pin Base; PAT отозваны. Backends работали в internal Docker network, без Docker socket, runner и внешних deploy targets. Human/JSON confirmations, пустые ответы, stdin, доступ/валидация/conflict, downloads/no-clobber проверены в QA и регрессиях. Оба порядка HTTP/wait timeout, задержки headers/body, redaction и cleanup download temp files проходят subprocess/HTTP tests.

Scoped restore исторического backup от 2026-10-01: три БД (Task Tracker 31, CI/CD 44, Wiki 17 таблиц), rows/sequences/constraints/owners/grants и четыре файловых volumes совпали с evidence; hashes и права 173 файлов совпали. После старта Task Tracker/Wiki данные и схема не изменились; все 8 SQLx checksums Wiki совпали с canonical source. Это не свежая согласованная копия текущего sdlc1.

**Блокеры rollout:** прежние постоянные runtime/images/volumes отсутствовали в Docker после восстановления окружения; их восстановление не входит в этот CLI rollout. Историческая CI/CD БД содержит применённые миграции 36/37, которых нет в опубликованном main (main содержит 1–35). Кандидат отказывается запускаться с VersionMissing(36). Существующий локальный commit `3aa12a4cdf077db720ef1e2e4c715db7fa443620` содержит эти файлы, но не включён в кандидат. Ledger миграций и данные не удалялись и не переписывались. Требуется отдельный план согласования опубликованного кода со схемой, свежий backup и проверенный rollback на прежние image IDs.

Исторические Wiki idempotency records не переписывались. Новый multipart hash подтверждён для записей после обновления; replay старых records с прежним raw multipart hash не сертифицирован, автоматической замены ключа нет. Wiki QA документы/space архивированы через API; до удаления временной QA-копии attachments/evidence/audit/replay retention rows оставались в ней. После проверки удалены только собственные QA ресурсы: 8 контейнеров, 7 volumes и 2 сети. Исходный protected backup и candidate images сохранены; исходные retention records не переписывались. Production pins, signing keys и volumes не изменялись.

**Статус:** исходники и три CLI-кандидата проверены; обновление sdlc1, прежние image rollback, живая приёмка обновлённого стенда и 15-минутное наблюдение не выполнены. QA CI/CD на пустой БД не заменяет приёмку сохранённых данных. Native Windows/macOS/musl, version bumps, tags и публичный release не входят в поставку.
