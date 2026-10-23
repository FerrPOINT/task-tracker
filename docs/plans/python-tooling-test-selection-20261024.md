# Точный выбор Python tooling unit

В рамках утверждённого плана PDLC: известные лёгкие helper test paths не должны
попадать в unknown fallback и выбирать frontend/backend builds.

1. Воспроизвести выбор реальным packaged selector.
2. Добавить exact ownership и отдельный Python helper gate; оставить существующий
   CI contract gate и unknown source script fallback.
3. Подключить те же unit modules к одной CI-команде, сохранив README validator.
4. Выполнить только затронутые Python units/static и публичный root PlanOnly.

Application/runtime/toolchains/pins не меняются; ручные integration/slow/release
остаются отдельными. POSIX-only/live checks этим gate не подменяются.

## Результат 24 октября

Реализованы exact test ownership, отдельный `tooling:python-helpers` и reviewed
`affected_checks`; существующий `tooling:build-contracts` и его argv сохранены.
CI запускает helper modules и CI contracts одной командой, README structural
validator остаётся самостоятельным. Все прежние artifact gates неизменны.

RED:3 regression tests, 15 subtest failures до исправления. GREEN:
**31 PASS за 6,007 с** для helper modules и существующего CI-contract
module. Вызовы packaged selector проверяют 4 известных test paths во всех
Fast/Unit/Build, ноль application producers, старый contract gate, unknown
scripts и реальные app inputs. Прямой запуск — команда из docs/TESTING.md плюс
`scripts.tests.test_ci_contract_checks`. Это source units, без Rust/Node/Docker.

Публичный root Unit/tooling PlanOnly:exit0, два лёгких tooling gates при явном
выборе всего компонента; changed-path выбор helper tests оставляет один gate.
Application builds/install/export0. Реальное ускорение сборки и CPU/RAM здесь
NOT_MEASURED. Quality/SDK/UI/skills/operator pins не менялись; перенос соседним
workspace требует проверки их актуальных descriptors, без массовой sync.
