# File Storage — Task Tracker

## 1. Overview

Вложения (attachments) хранятся в локальной файловой системе через `FileStorage`. S3-compatible storage, аватары и export-файлы описаны как будущие расширения.

## 2. Supported Backends

| Backend | Use Case |
|---------|----------|
| `filesystem` | Local dev, single-node deploy; реализовано |
| `s3` | Production, scalable, backups; future |
| `minio` | Self-hosted S3-compatible; future |

## 3. Configuration

```env
TASKTRACKER_STORAGE__DIR=/data/attachments
TASKTRACKER_STORAGE__MAX_UPLOAD_BYTES=26214400
```

## 4. FileStore Trait

```rust
#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn put(&self, issue_id: &str, key: &str, bytes: Vec<u8>) -> Result<(), AppError>;
    async fn get(&self, issue_id: &str, key: &str) -> Result<Vec<u8>, AppError>;
    async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError>;
}
```

## 5. Attachment Flow

### 5.1 Upload

1. Client POST `/api/v1/issues/{id}/attachments` multipart/form-data.
2. Server валидирует:
   - max size (default 25 MiB);
   - whitelist заявленного content-type;
   - filename sanity (path/control символы).
3. Server генерирует `attachment_id` UUID.
4. Файл сохраняется в storage под ключом `{issue_id}/{uuid}-{sanitized_filename}`.
5. Запись в `attachments` таблице.
6. Возвращается `AttachmentResponse`.

### 5.2 Download

1. GET `/api/v1/attachments/{attachment_id}/download`.
2. Server проверяет права (project access).
3. Возвращает файл как `application/octet-stream` или redirect на signed S3 URL.

### 5.3 Thumbnails

- Для изображений генерируются thumbnails (max 256x256).
- Thumbnail ключ: `thumbnails/{attachment_id}.webp`.
- Генерация в фоновом job (`apalis`) через `image` crate.

## 6. Attachment Entity

```rust
pub struct Attachment {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub filename: String,
    pub storage_key: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub thumbnail_key: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}
```

## 7. Security

- Запрещённые типы: executable, script files.
- Scan on upload via ClamAV (optional async scan).
- Quarantine bucket/file если scan positive.
- Filename sanitized: удаляются `..`, null bytes, control chars.
- Content-Type проверяется по whitelist заявленного multipart content-type.
- Magic bytes validation — future.

## 8. Virus Scanning

```rust
pub trait VirusScanner: Send + Sync {
    async fn scan(&self, bytes: Bytes) -> Result<ScanResult, ScanError>;
}
```

- Интеграция с ClamAV (`clamd`).
- Загруженный файл помечается `scanned_at`.
- Если заражён — удаляется и логируется.

## 9. Avatars

- User avatars: `avatars/{user_id}.webp`.
- Project avatars: `avatars/projects/{project_id}.webp`.
- Max size 2 MB, форматы jpg/png/webp.

## 10. Export / Import Files

- CSV/JSON export временно сохраняется в storage.
- Key: `exports/{user_id}/{export_id}.json`.
- TTL 24 часов, cleanup job.

## 11. Cleanup Job

- Daily cleanup удаляет orphaned attachments (нет записи в БД или issue удалён).
- Hard delete после 30 дней в soft-delete режиме.

## 12. S3 Signed URLs

Для production:

- Download через signed URL (TTL 15 минут).
- Server генерирует URL, не проксирует большие файлы.

```rust
fn signed_url(&self, key: &str, expires_in: Duration) -> String
```

## 13. Quotas

| Entity | Default Limit |
|--------|---------------|
| Per attachment | 25 MiB |
| Total per issue | 500 MB |
| Total per project | 10 GB |
| Avatar | 2 MB |

## 14. API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/issues/{id}/attachments` | Upload attachment |
| GET | `/api/v1/issues/{id}/attachments` | List issue attachments |
| GET | `/api/v1/attachments/{id}/download` | Download attachment |
| DELETE | `/api/v1/attachments/{id}` | Delete attachment |

## 15. Storage Path Schema

```
{backend-specific prefix}/
  {issue_id}/{uuid}-{sanitized_filename}
```

## 16. Backup

- S3 bucket с versioning + lifecycle policy — future.
- Filesystem — backup Docker volume / storage directory.
- Restore: sync из backup + проверка consistency с `attachments` таблицей.
## References

- `docs/ARCHITECTURE.md`
- `docs/DATA_MODEL.md`
- `docs/SECURITY.md`
