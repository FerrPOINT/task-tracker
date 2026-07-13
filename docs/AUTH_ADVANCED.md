# Auth Advanced — Task Tracker

## 1. Overview

Дополнительные механизмы безопасности аутентификации и сессий.

## 2. Refresh Token Rotation

- При каждом refresh выдаётся **новая пара** access + refresh tokens.
- Старый refresh token становится недействительным.
- Refresh token хранится hashed (`SHA-256`) в БД.

## 3. Refresh Token Reuse Detection

- Если использован ранее отозванный refresh token:
  - Отклонить запрос.
  - Инвалидировать всю token family (все refresh tokens пользователя).
  - Отправить security email пользователю.
  - Залогировать в `audit_log`.

## 4. Session Invalidation Triggers

Сессия/refresh tokens инвалидируются при:

- Смене пароля.
- Смене email.
- Logout.
- Административном блокировании пользователя.
- Подозрении на compromise (reuse detection).

## 5. Rate Limiting

| Endpoint | Limit |
|----------|-------|
| `POST /auth/register` | 5 attempts / IP / hour |
| `POST /auth/login` | 10 attempts / IP / 5 min |
| `POST /auth/forgot-password` | 3 attempts / email / hour |
| `POST /auth/reset-password` | 5 attempts / IP / hour |
| `POST /auth/refresh` | 30 attempts / token / 10 min |
| General API | 100 req / user / min, 1000 req / IP / min |

Реализация: Redis token bucket.

## 6. Password Security

- argon2id с параметрами:
  - memory: 19 MB
  - iterations: 2
  - parallelism: 1
- Минимальная длина: 10 символов.
- Проверка на common passwords (top 10000).
- Опционально: проверка через HIBP API.

## 7. Account Lockout

- После 5 неудачных login attempts — lockout на 15 минут.
- Уведомление на email.
- Сброс только через email link или admin action.

## 8. Email Verification

- Новый email не активен до подтверждения.
- Verification token TTL: 24 часа.
- Повторная отправка письма ограничена 3 разами / час.

## 9. Device / Session Tracking

- `sessions` хранит:
  - user agent hash
  - IP address
  - created_at
  - last_used_at
- UI позволяет пользователю просматривать и отзывать сессии.

## 10. Audit Events

Все auth события пишутся в `audit_log`:

- login_success
- login_failure
- logout
- password_changed
- email_changed
- refresh_token_rotated
- refresh_token_reused
- account_locked
- account_unlocked

## 11. API Tokens (Service Accounts)

- Отдельный тип токена для CLI/integrations.
- Хранится hashed в `api_tokens`.
- Поддерживает expiration и scope.
- Отображается только при создании.

## References

- `docs/SECURITY.md`
- `docs/API.md`
- `docs/DATA_MODEL.md`
- `docs/EVENTS.md`
- `docs/TESTING.md`
