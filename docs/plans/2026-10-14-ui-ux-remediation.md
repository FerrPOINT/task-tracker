# Доработки UI/UX 2026-10-14

UX-06/09. Создание первого проекта на dashboard и в форме задачи использует существующий ProjectFormDialog. После подтверждения каталог обновляется, проект выбирается, ввод задачи сохраняется; отказ создания остаётся в диалоге. Вход и callback SSO используют Base.

Общие контракты и тексты: [стандарт Base](https://github.com/FerrPOINT/services-base/blob/16e0ab44924fae7c67c2df56438667e7830cafd6/docs/platform/UI_UX_STANDARD.md). UI pin: `16e0ab44924fae7c67c2df56438667e7830cafd6`; SDK и skills pins сохранены.

Frontend: 385 тестов; TypeScript/lint, build, frozen lockfile и OpenAPI. IAB: общий анонимный вход на 320/375/1920/2560.

Зависимость: [Base PR #206](https://github.com/FerrPOINT/services-base/pull/206). Кандидат поставляется draft: полная межпродуктовая IAB-приёмка, SSO и 30 направлений переключения ещё не подтверждены. Backend не менялся; его полный локальный Rust gate был остановлен после ограничения на дополнительные Docker-группы. GitHub CI учитывается отдельно; отсутствие запуска из-за billing не объявляется PASS.

Работа и native QA выполняются в PDLC2 без новых Docker-групп. Merge и установка исключены; установка — отдельная задача.

## Генерализация UI

UserAvatar, NavigationDrawer и оформление приоритета используют Base. Приоритеты сопоставляются в единственном адаптере Tracker.
