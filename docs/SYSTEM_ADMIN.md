# System Administration — Task Tracker

## 1. Users

### 1.1 User Fields

| Поле | Описание |
|------|----------|
| `id` | UUIDv7 |
| `username` | Уникальный логин |
| `email` | Email |
| `display_name` | Отображаемое имя |
| `avatar_url` | URL аватара |
| `active` | Активен ли пользователь |
| `locale` | `ru` или `en` |
| `timezone` | Таймзона |
| `created_at` | Дата создания |
| `last_login_at` | Последний вход |

### 1.2 User Management

- Учётными записями, установкой пароля, отключением и восстановлением владеет
  Central Auth; операции доступны в Admin Panel.
- Task Tracker читает центральный каталог для назначения исполнителей и создаёт
  локальный профиль при первом входе или первом назначении.
- Локальная страница управления пользователями и локальные create/disable API
  удалены. Self-registration, password login/reset и TOTP отключены при заданном
  `TT_AUTH__CENTRAL_JWKS_URI`.
- Email действующей центральной учётки не редактируется в Task Tracker.

## 2. Groups

Группы ниже являются исторической целевой моделью. В центральном режиме они не
ограничивают вошедших пользователей и экраны их назначения не показываются.

| Группа | Описание |
|--------|----------|
| `jira-administrators` | System Admin |
| `jira-users` | Все залогиненные пользователи |
| `jira-servicedesk-users` | Агенты service desk (если включено) |

### 2.1 Group Management

- Создание групп.
- Добавление/удаление пользователей.
- Привязка групп к global permissions.

## 3. Global Permissions

Поля разрешений сохраняются для совместимости legacy-данных. В центральном
режиме отдельные пользовательские роли не назначаются.

| Permission | Описание |
|-----------|----------|
| `system_admin` | Доступ к System Admin |
| `administer_users` | Управление пользователями |
| `administer_groups` | Управление группами |
| `create_project` | Создание проектов |
| `bulk_change` | Массовые изменения |
| `share_filters` | Шаринг фильтров |
| `share_dashboards` | Шаринг дашбордов |

## 4. Authentication

### 4.1 Local Auth

- Работает только в legacy-режиме без `TT_AUTH__CENTRAL_JWKS_URI`.
- При включённом Central Auth локальные register/login/refresh/password/TOTP
  маршруты fail closed и не используются как fallback.

### 4.2 OAuth2 / OIDC

Платформенный вход реализован через Central Auth Authorization Code + PKCE:

- UI использует публичный issuer и хранит access token только в памяти;
  центральная refresh/session cookie остаётся HttpOnly.
- Backend проверяет подпись, issuer, audience, срок и активность сессии через
  внутренний адрес Central Auth.
- Локальный профиль связан с `central_sub`; совпадение email не является
  доказательством идентичности.
- Глобальный выход отзывает центральную браузерную сессию во всех приложениях,
  но не отзывает долгоживущие личные API-токены.

### 4.3 LDAP / Active Directory

- Синхронизация пользователей и групп.
- Login via bind DN.

## 5. Application Settings

### 5.1 General

| Настройка | Значение по умолчанию |
|-----------|----------------------|
| `application_title` | Task Tracker |
| `base_url` | `http://localhost:3456/api/v1` |
| `default_locale` | `ru` |
| `default_timezone` | `Europe/Moscow` |
| `date_format` | `dd/MM/yyyy` |
| `datetime_format` | `dd/MM/yyyy HH:mm` |

### 5.2 Instance Limits

- Максимальное количество проектов.
- Максимальное количество пользователей.
- Максимальный размер вложения.
- Rate limits.

## 6. Email Server

### 6.1 SMTP Settings

| Поле | Описание |
|------|----------|
| `smtp_host` | Хост SMTP |
| `smtp_port` | Порт |
| `smtp_username` | Логин |
| `smtp_password` | Пароль |
| `smtp_use_tls` | TLS |
| `from_address` | Адрес отправителя |
| `from_name` | Имя отправителя |

### 6.2 Test Email

- Кнопка "Send test email" для проверки конфигурации.

## 7. Security

### 7.1 Password Policy

- Минимальная длина: 12 символов.
- Требование букв, цифр, спецсимволов.
- Срок действия пароля (опционально).
- История паролей.

### 7.2 Session Policy

- Автоматический logout при бездействии.
- Ограничение на количество сессий.
- Принудительный logout всех пользователей.

### 7.3 Audit Log

- Все admin-действия записываются.
- Не подлежит удалению.
- Фильтрация по пользователю, типу события, дате.

## 8. Backup and Restore

### 8.1 Automated Backup

- Ежедневный backup БД и attachments.
- Хранение N последних копий.
- S3 / local volume.

### 8.2 Manual Backup

- Кнопка "Create backup" в System Admin.
- JSON dump всего инстанса.

### 8.3 Restore

- Restore из backup (только System Admin).
- Merge / replace режимы.

## 9. Plugins / Extensions

### 9.1 Plugin System

- Rust plugin API (WASM-compatible).
- Frontend plugin slots.
- Marketplace (опционально).

### 9.2 Webhooks

- Глобальные webhooks на системные события.
- Payload signing (HMAC-SHA256).

## 10. Monitoring and Logs

### 10.1 Metrics

- Prometheus `/metrics`.
- OpenTelemetry traces — не реализовано (см. MONITORING.md).

### 10.2 Health Checks

| Endpoint | Описание |
|----------|----------|
| `/health` | Liveness |
| `/health/ready` | Readiness (DB, Redis) |
| `/health/metrics` | Prometheus metrics |

### 10.3 Log Levels

- `error`, `warn`, `info`, `debug`, `trace`.
- Структурированные JSON-логи.

## 11. Maintenance

### 11.1 Re-index

- Полный reindex поиска.
- Фоновая задача.

### 11.2 Data Integrity Check

- Проверка orphaned attachments.
- Проверка consistency workflow / statuses.

### 11.3 License

- Self-hosted: no license required.
- Enterprise: license key + feature flags.
## References

- `docs/ARCHITECTURE.md`
- `docs/SECURITY.md`
- `docs/I18N.md`
