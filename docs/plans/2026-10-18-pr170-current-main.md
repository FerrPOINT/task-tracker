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

Текущий статус: исходники подготовлены; до публикации нужны профильные проверки
Rust/PostgreSQL, frontend typecheck/tests/build и OpenAPI. Старая CI-квалификация
исходного PR и тесты отдельных родителей не считаются проверкой этого дерева.
Приёмка dev SSO/CRUD и scheduler остаётся отдельным этапом у владельца стенда.
