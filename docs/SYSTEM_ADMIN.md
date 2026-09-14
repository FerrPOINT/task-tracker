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
- Password reset по email — реализовано: `POST /api/v1/auth/password/request`
  (202 всегда; письмо с одноразовой ссылкой 30 минут) + `POST /api/v1/auth/password/reset`
  (token + новый пароль; refresh-сессии отзываются). Базовый URL ссылки —
  `TASKTRACKER_RESET_BASE_URL` (default `http://localhost:5173`); отправка через
  SMTP-конфигурацию `email.*` (при `email.enabled=false` письмо не уходит, токен
  всё равно выпускается — для dev/staging).
- Change password / email.
- Two-factor authentication (TOTP) — опционально, self-service: authenticated
  user starts enrollment via `POST /api/v1/auth/totp/setup`, scans `otpauth_uri`,
  confirms with `POST /enable` and stores the returned recovery codes offline.
  `POST /disable` requires a current TOTP or an unused recovery code.

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

Реализовано (single provider, authorization code + PKCE S256):

- Настройка через env: `TASKTRACKER_OIDC_ISSUER_URL` (пусто = SSO выключен), `TASKTRACKER_OIDC_CLIENT_ID`, `TASKTRACKER_OIDC_CLIENT_SECRET`, `TASKTRACKER_OIDC_REDIRECT_URL` (default `http://localhost:7721/api/v1/auth/oidc/callback`).
- `GET /api/v1/auth/oidc/begin` → 302 на authorization endpoint провайдера (state + nonce + PKCE, single-use state в БД, TTL 10 минут).
- `GET /api/v1/auth/oidc/callback?state&code` → обмен кода на id_token, проверка nonce; связывание по (provider, sub); существующий локальный email привязывается, иначе JIT-провижининг (неактивный локальный пароль `!`); выдача локальных access/refresh токенов как при обычном логине.
- Identity-линки хранятся в `oidc_identities` (миграция 0032); states — в `oidc_state`, single-use.
- Поддерживаются OIDC-совместимые провайдеры: rauthy (стенд), Keycloak, Google, GitHub (OIDC-приложения).
- SAML 2.0 — опционально, не реализовано (future).

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
