# Общие диалоги и завершение подключения Base

Выделить исправления диалогов из прежнего интеграционного кандидата в ветку
от принятого main. PM routes, DTO, migrations, grants и серверные Drafts
не входят в этот PR.

Существующие общие Dialog/ConfirmDialog Base предоставляют focus/pending/error
механизмы. Продукт сохраняет запись worklog, права, календарь и duration parser,
включая дробные значения. После удаления строки focus возвращается на вкладку
worklog; после отмены — на инициатор. Диалог остаётся mounted при пустом списке.
Pending формы блокирует повторную отправку и закрытие. Новый dialog очищает
ошибку прошлого действия. Эквивалентные let chains обеспечивают strict Clippy
на принятом Rust 1.88.0 без изменения бизнес-правил.

Проверки: locked Rust workspace/Clippy/domain/OpenAPI, реальные PostgreSQL
repositories/FK/profile/restore; frozen frontend typecheck/lint/format/tests/build,
OpenAPI compatibility, packed Base consumer и effective themes. Общая приёмка
повторяет production dialogs, keyboard/focus, async состояния, темы и viewport
на точных commits всего комплекта. Прежние снимки сохранены как исторические
доказательства, новая общая поставка проверяется отдельно.
