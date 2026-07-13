# File Storage — Task Tracker

## 1. Overview

Вложения (attachments, аватары, экспорт-файлы) хранятся в S3-compatible storage или локальной файловой системе. Backend абстрагирует хранилище через `FileStore` trait.

## 2. Supported Backends

| Backend | Use Case |
|---------|----------|
| `filesystem` | Local dev, single-node deploy |
| `s3` | Production, scalable, backups |
| `minio` | Self-hosted S3-compatible |

## 3. Configuration

```env
TASKTRACKER_FILE_STORAGE_BACKEND=s3
TASKTRACKER_FILE_STORAGE_BUCKET=tasktracker-attachments
TASKTRACKER_FILE_STORAGE_REGION=ru-central1
TASKTRACKER_FILE_STORAGE_ENDPOINT=https://s3.example.com
TASKTRACKER_FILE_STORAGE_ACCESS_KEY=...
TASKTRACKER_FILE_STORAGE_SECRET_KEY=...
TASKTRACKER_FILE_STORAGE_PATH=/data/attachments  # for filesystem
```

## 4. FileStore Trait

```rust
#[async_trait]
pub trait FileStore: Send + Sync {
    async fn put(&self, key: &str, content: Bytes, content_type: &str) -> Result<(), FileStoreError>;
    async fn get(&self, key: &str) -> Result<Bytes, FileStoreError>;
    async fn delete(&self, key: &str) -> Result<(), FileStoreError>;
    async fn exists(&self, key: &str) -> Result<bool, FileStoreError>;
    fn public_url(&self, key: &str) -> String;
}
```

## 5. Attachment Flow

### 5.1 Upload

1. Client POST `/api/v1/issues/{id}/attachments` multipart/form-data.
2. Server валидирует:
   - max size (default 50 MB);
   - mime-type whitelist;
   - filename sanity (path traversal).
3. Server генерирует `attachment_id` UUIDv7.
4. Файл сохраняется в storage под ключом `attachments/{issue_id}/{attachment_id}/{filename}`.
5. Запись в `attachments` таблице.
6. Возвращается `AttachmentResponse`.

### 5.2 Download

1. GET `/api/v1/attachments/{attachment_id}`.
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
- Content-Type не доверяем blindly; определяем по magic bytes.

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
| Per attachment | 50 MB |
| Total per issue | 500 MB |
| Total per project | 10 GB |
| Avatar | 2 MB |

## 14. API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/issues/{id}/attachments` | Upload attachment |
| GET | `/api/v1/attachments/{id}` | Download attachment |
| GET | `/api/v1/attachments/{id}/thumbnail` | Download thumbnail |
| DELETE | `/api/v1/attachments/{id}` | Delete attachment |
| GET | `/api/v1/users/{id}/avatar` | User avatar |
| POST | `/api/v1/users/me/avatar` | Upload avatar |

## 15. Storage Path Schema

```
{backend-specific prefix}/
  attachments/{issue_id}/{attachment_id}/{sanitized_filename}
  thumbnails/{attachment_id}.webp
  avatars/users/{user_id}.webp
  avatars/projects/{project_id}.webp
  exports/{user_id}/{export_id}.json
```

## 16. Backup

- S3 bucket с versioning + lifecycle policy.
- Filesystem — rsync к backup volume.
- Restore: sync из backup + проверка consistency с `attachments` таблицей.
