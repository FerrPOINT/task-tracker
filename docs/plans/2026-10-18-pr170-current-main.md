# Совместимость PR170 с опубликованным восстановлением PM

Кандидат объединяет PR170 `f815d20d` с main `73f7bd25`. Обе уже существующие
0098 сохраняют полные SeaORM migration names: восстановление PM и проектный
каталог являются разными записями. Применённые SQL и ledger не переписываются.
На published/fresh lineage PM recovery зарегистрирован перед project catalog;
историческая project-repositories lineage остаётся отдельной строгой ветвью.

Populated upgrade проверяет обе миграции по одной: прежние ID/receipts/history,
пустой каталог, отказы destructive down и повторный up. PM recovery test сохраняет
собственный этап 0097→PM0098 и отдельно допускает следующий project catalog.
Добавлены published и legacy БД, где project catalog применён до PM recovery:
связь проекта и прежние ledger rows должны сохраниться после дополнения.

CI явно исполняет все пять ignored project compatibility tests в пяти
свежих отдельных БД, с exact selector и проверкой одного выполненного теста.
Существующий отдельный PM PostgreSQL gate сохраняется. Новый UI pin принимает
Base262 `e522088b`: текущий main Base, basename и callback transport. SDK pin
не меняется. Runtime и пользовательские данные этой подготовкой не изменяются.

Квалификация завершена: 772 обычных Rust-теста и strict workspace clippy
на неизменившихся production inputs; девять отдельных PostgreSQL/HTTP сценариев,
включая пять историй миграций, пагинацию, PM recovery, populated upgrade и HTTP.
Финальная дельта backend после общего suite — ignored HTTP fixture; она отдельно
исполнена с PostgreSQL и strict clippy. Существующий Authz.require_writable
подтверждён для архивного проекта без дополнительной политики в обработчике.

Frontend: 499 тестов, typecheck, lint, format, OpenAPI check/compat, root и /tracker/
build. Chromium UI fixture проверяет 100→2→100, отсутствие горизонтального
переполнения и full-page screenshots 375/1920/2560. Fixture содержит синтетические
данные и не является real SSO acceptance. На Windows прошли 14 docs/CI checks;
четыре неизменённые POSIX wrapper checks здесь не исполнялись.

Исходный дефект воспроизведён production repository: из 102 связей видны только
100. Теперь bounded offset paging сохраняет полный каталог, порядок и project
scope; UI сбрасывает страницу при смене проекта и позволяет вернуться после
ошибки поздней страницы. Исходники и runtime соседней разработки не менялись.

Живые SSO/CRUD и scheduler acceptance новой версии остаются отдельным этапом.
Кандидат здесь не установлен; Draft зависит от Base262 и интеграционной приёмки.
