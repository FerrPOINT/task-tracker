# Feature Flags — Task Tracker

## 1. Overview

Механизм включения/выключения функционала без деплоя.

## 2. Flag Sources

| Source | Priority | Use Case |
|--------|----------|----------|
| Env var | Highest | Emergency kill switch |
| DB `feature_flags` | Medium | Gradual rollout |
| Per-user override | Lowest | Beta testing |

## 3. Data Model

```sql
CREATE TABLE feature_flags (
  id UUID PRIMARY KEY,
  key TEXT NOT NULL UNIQUE,
  enabled BOOLEAN NOT NULL DEFAULT false,
  rollout_percentage INT NOT NULL DEFAULT 0 CHECK (rollout_percentage BETWEEN 0 AND 100),
  allowed_user_ids UUID[],
  description TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

## 4. Evaluation

```rust
pub fn is_enabled(flag: &str, user_id: Option<Uuid>) -> bool {
    // env override
    if let Ok(v) = std::env::var(format!("TASKTRACKER_FF_{}", flag.to_uppercase())) {
        return v == "true" || v == "1";
    }
    // db flag
    let config = self.repo.get(flag);
    if !config.enabled { return false; }
    if config.rollout_percentage == 100 { return true; }
    if let Some(uid) = user_id {
        if config.allowed_user_ids.contains(&uid) { return true; }
        let hash = hash_user_flag(uid, flag);
        return hash % 100 < config.rollout_percentage;
    }
    false
}
```

## 5. Built-in Flags

| Flag | Description |
|------|-------------|
| `webhooks` | Webhook integrations |
| `email_to_issue` | Email-to-issue processing |
| `advanced_reports` | Cumulative flow, control chart |
| `custom_field_rules` | Field-level validators |
| `public_boards` | Read-only public boards |
| `plugin_system` | Plugin API |

## 6. Frontend Integration

- `GET /api/v1/features` возвращает enabled flags для текущего пользователя.
- Frontend кэширует на сессию.
- Компонент `FeatureFlag name="webhooks" fallback={null}>`.

## 7. Testing

- Unit tests для `FeatureFlagService`.
- E2E тесты с фиксированными флагами через seed.

## References

- `docs/DATA_MODEL.md`
- `docs/API.md`
- `docs/TESTING.md`
