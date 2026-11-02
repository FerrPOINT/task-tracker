# Общая подпись архивного проекта

Заменить native span с копией шрифта/цвета на ResourceListDescription asChild
Base. Native span и block placement сохраняются. Не менять admission чтения,
поиск единственного namespace binding, archived state, имя проекта или URL.
Pending, error, отсутствие и неоднозначность binding не выбирают первый ресурс.

Используется принятый UI producer fd598ba5c977b9f16855cc619983eb0415db9623,
без установки зависимостей, обновления SDK/skills и повторной сборки Base.
Проверки: whole-module source projection с отрицательным изменением owner field,
scoped types/lint/UI guard, project/actions tests. Визуальная приёмка остаётся
открытой до Codex IAB; установленный runtime и production не меняются.


Проверено локально: typecheck, lint, UI contract guard и 21 project/actions
case. Whole-module projection совпадает после снятия только caption wrapper;
изменение resource_id отклоняется. UI pin, SDK/skills, API и режимы admission
сохранены; browser gate остаётся открытым.
