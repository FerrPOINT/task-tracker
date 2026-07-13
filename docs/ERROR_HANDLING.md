# Error Handling — Task Tracker

## 1. Overview

Единая стратегия обработки ошибок на backend и frontend: отличать ожидаемые бизнес-ошибки от неожиданных технических, не дать утечь деталям инфраструктуры наружу, дать пользователю понятное сообщение.

## 2. Backend Error Hierarchy

### 2.1 Error Types

```rust
pub enum DomainError {
    NotFound { entity: &'static str, id: String },
    AlreadyExists { entity: &'static str, key: String },
    Validation { field: String, message: String },
    PermissionDenied { action: String },
    InvalidTransition { from: String, to: String },
    Conflict { message: String },
}

pub enum AppError {
    Domain(DomainError),
    Infra(InfraError),
    Unauthorized,
    Forbidden,
}

pub enum InfraError {
    Database(sqlx::Error),
    Redis(redis::RedisError),
    External(String),
    Config(String),
    Internal(String),
}
```

### 2.2 HTTP Mapping

| AppError | HTTP Status | User-facing code |
|----------|-------------|------------------|
| `Validation` | 400 | `VALIDATION_ERROR` |
| `Unauthorized` | 401 | `UNAUTHORIZED` |
| `Forbidden` / `PermissionDenied` | 403 | `FORBIDDEN` |
| `NotFound` | 404 | `NOT_FOUND` |
| `AlreadyExists` / `Conflict` | 409 | `CONFLICT` |
| `InvalidTransition` | 422 | `INVALID_TRANSITION` |
| `Infra::External` | 502 | `EXTERNAL_ERROR` |
| `Infra::Internal` | 500 | `INTERNAL_ERROR` |
| other | 500 | `INTERNAL_ERROR` |

### 2.3 Response Format

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Request validation failed",
    "requestId": "req-uuid",
    "details": [
      { "field": "summary", "message": "required" }
    ]
  }
}
```

### 2.4 Middleware

- `TraceLayer` — логирует все запросы с request_id.
- `CatchPanicLayer` — превращает panic в 500.
- Глобальный обработчик AppError → Response.
- Логирование: `error` логируется с полным контекстом; в ответе — без stack trace.

## 3. Frontend Error Handling

### 3.1 API Errors

```ts
// shared/api/handleApiError.ts
export class ApiError extends Error {
  constructor(
    public code: string,
    public status: number,
    public details?: Array<{ field: string; message: string }>
  ) {
    super(code)
  }
}

export function handleApiError(error: unknown): ApiError {
  if (axios.isAxiosError(error)) {
    const data = error.response?.data?.error
    return new ApiError(data?.code || "UNKNOWN", error.response?.status || 0, data?.details)
  }
  return new ApiError("UNKNOWN", 0)
}
```

### 3.2 Query Error Handling

```ts
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (failureCount, error) => {
        const apiError = handleApiError(error)
        return apiError.status >= 500 && failureCount < 3
      },
      meta: {
        errorMessage: "Failed to load data",
      },
    },
    mutations: {
      onError: (error) => {
        const apiError = handleApiError(error)
        toast.error(apiError.message)
      },
    },
  },
})
```

### 3.3 Error Boundaries

```tsx
// shared/ui/error-boundary.tsx
export class ErrorBoundary extends React.Component {
  state = { hasError: false }
  static getDerivedStateFromError() {
    return { hasError: true }
  }
  componentDidCatch(error: Error, info: React.ErrorInfo) {
    logError(error, info)
  }
  render() {
    if (this.state.hasError) {
      return <ErrorState onRetry={() => this.setState({ hasError: false })} />
    }
    return this.props.children
  }
}
```

Размещение:

- App-level boundary — при критических ошибках.
- Page-level boundary — при ошибках загрузки страницы.
- Widget-level boundary — при ошибках виджета (dashboard gadget).

## 4. Form Errors

- Ошибки валидации сервера мапятся на поля формы.
- Общие ошибки — баннер над формой.
- Пример:

```ts
onError: (error) => {
  const apiError = handleApiError(error)
  if (apiError.status === 400) {
    apiError.details?.forEach(({ field, message }) => {
      form.setError(field as Path<FormValues>, { type: "server", message })
    })
  } else {
    toast.error(apiError.message)
  }
}
```

## 5. Retry Strategies

### 5.1 Backend

- DB connections: exponential backoff через `sqlx::Pool`.
- External HTTP calls: 3 retries.
- Redis: reconnect.

### 5.2 Frontend

- Queries: retry только для 5xx.
- Mutations: no retry.
- WebSocket: exponential reconnect до 30s max.

## 6. Request ID

- Каждый запрос получает `x-request-id`.
- Передаётся во все сервисы.
- Frontend показывает `requestId` в сообщении об ошибке для support.

## 7. Known Error Scenarios

| Scenario | Backend | Frontend |
|----------|---------|----------|
| Invalid login | 401 `INVALID_CREDENTIALS` | toast + форма |
| Duplicate project key | 409 `PROJECT_KEY_EXISTS` | inline field error |
| Issue not found | 404 `ISSUE_NOT_FOUND` | 404 page |
| Transition not allowed | 422 `INVALID_TRANSITION` | toast |
| DB unavailable | 500 `INTERNAL_ERROR` | retry + fallback page |
| Network error | — | toast + offline badge |
| WS disconnect | — | reconnect spinner |

## 8. Logging

- Все ошибки уровня `error`/`warn` пишутся в JSON.
- Поля: `level`, `message`, `request_id`, `user_id`, `trace`, `target`.
- Panic логируется через `CatchPanicLayer`.

## 9. Alerting

- 5xx rate > 1% → alert.
- DB connection errors → critical alert.
- External service downtime → warning.

## 10. User-Facing Messages

| Code | Russian | English |
|------|---------|---------|
| `VALIDATION_ERROR` | Проверьте введённые данные | Please check your input |
| `UNAUTHORIZED` | Требуется вход в систему | Please sign in |
| `FORBIDDEN` | Недостаточно прав | Permission denied |
| `NOT_FOUND` | Объект не найден | Not found |
| `CONFLICT` | Конфликт данных | Data conflict |
| `INVALID_TRANSITION` | Невозможный переход | Invalid transition |
| `INTERNAL_ERROR` | Внутренняя ошибка. Попробуйте позже | Internal error. Please try again later |
| `EXTERNAL_ERROR` | Внешняя служба недоступна | External service unavailable |
| `RATE_LIMITED` | Слишком много запросов | Too many requests |
## References

- `docs/ARCHITECTURE.md`
- `docs/API.md`
