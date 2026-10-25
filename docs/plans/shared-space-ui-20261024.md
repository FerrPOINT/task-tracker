# Общие панели пространств и состояния уведомлений

## Общее меню аккаунта — 25 октября 2026

Локальное identity/theme/logout menu заменено AccountMenu. Nullish fallback имени, условие secondary email, logout.mutate/pending, modal=false и ACL/admin item сохраняются у Tracker.

UI producer `b50b7f53ea0b69f957b2babed8147129176fff03`, reference
`e30f5c7ee0b4690fd7f5f7df1caef73c1f478c70a1eb407db08ef90a14b09b1b`. Все семь UI pins согласованы;
шесть frozen offline refresh сохранили locks и React singleton. Независимые
SDK/skills pins не менялись. UI 0.3.2, reference schema 2 и API v1 сохранены.

Base: 16 distinct UI cases, 43 primitive guard cases, 72 reference cases,
types/examples/catalog/lint и настоящий offline UI-архив PASS. Шесть consumer
types и guards, четыре lint изменённых owners и 137 distinct consumer cases
(Admin 12, Tracker 38, Wiki 23, Forge 18, Fleet 25, AI Hub 21) PASS.
Whole-module/TS proof сохраняет остальную программу; шесть негативных контролей
проверяют pending, secondary, logout, route, ACL и AppShell forwarding.

IAB: 96 source-контекстов четырёх точных menu adapters, три темы,
320/375/1920/2560 и standard/large. Дополнительно проверены theme-only,
pending/ACL, owner links, однократный logout, modal/inert, клавиатура и возврат
фокуса. Это синтетические identity/actions без backend/SSO, не installed
acceptance. Source hashes неизменны; временный процесс закрыт, продуктовых
запросов и console errors нет. Снимки 375/1920/2560 имеют реальные размеры.

Evidence: private `evidence/account-menu-20261025/`. Архив: 822 файла,
1 149 993 байт, SHA256
`a03bb855d4b6865e9f11a8ccdb837a5a97e99f0e20f681b949a0712068223161`.
Source publication, hosted CI, общий integrated gate и установленная приёмка
этой scoped волной не заявляются. Предыдущие разделы сохраняют исторические
producer и результаты, а не описывают текущий UI pin.

Baseline — `db360b9b178482f5fd29d1b1ae007d822221f4dd`. Используются уже
доступные контракты UI pin `cf1e76e4be37f3611806f48b4d1b75d0470d78b0`.

Панели неподтверждённой команды и проекции Wiki, а также выбранного пространства
переходят на Card/CardContent. Собственная разметка loading/empty и пагинации
уведомлений заменяется LoadingState, EmptyState и PageNavigation Base.

Запросы, admission, сохранение набора пространств, CAS, operation key, readback,
сброс фильтра и URL уведомлений остаются у Tracker. Выбор checkbox с подробным
описанием требует отдельного общего контракта; этот план не объявляет его
мигрированным. Нативный file input вложений — технический контроль загрузки.

Проверки: scoped tests двух форм пространств и уведомлений, TypeScript и lint
изменённых файлов, guards Base. Полный интеграционный и браузерный gate — после
сведения следующей порции. Источники не означают установку; runtime не меняется.

Scoped результат: 31 test в трёх файлах прошёл; TypeScript, scoped ESLint,
shared choice/primitive guards и CLI check принятого UI прошли. AST трёх
production-файлов подтверждает сохранность логики вне JSX и критичных вызовов.
Заголовки и состояния формы связей также используют SectionHeader/Notice;
допуск и подтверждение Wiki не смешиваются с цветом или статусом успеха.
