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

- Создание / блокировка / удаление пользователя.
- Bulk import из CSV.
- Self-registration (опционально, отключается в настройках).
- Password reset по email.
- Change password / email.
- Two-factor authentication (TOTP) — опционально.

## 2. Groups

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

- Argon2id для хеширования паролей.
- JWT access token (TTL 15 минут).
- httpOnly refresh cookie (TTL 7 дней).

### 4.2 OAuth2 / OIDC

- Настраиваемые OAuth2 провайдеры (Google, GitHub, Keycloak, etc.).
- SAML 2.0 — опционально.

### 4.3 LDAP / Active Directory

- Синхронизация пользователей и групп.
- Login via bind DN.

## 5. Application Settings

### 5.1 General

| Настройка | Значение по умолчанию |
|-----------|----------------------|
| `application_title` | Task Tracker |
| `base_url` | `http://localhost:19876` |
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
- OpenTelemetry traces.

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
