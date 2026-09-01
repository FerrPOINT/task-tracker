use shared::{AppError, StorageConfig};
use std::path::PathBuf;

#[async_trait::async_trait]
impl domain::FileStorage for FileStorage {
    async fn put(&self, issue_id: &str, key: &str, bytes: Vec<u8>) -> Result<(), AppError> {
        FileStorage::put(self, issue_id, key, bytes)
            .await
            .map(|_| ())
    }
    async fn get(&self, issue_id: &str, key: &str) -> Result<Vec<u8>, AppError> {
        FileStorage::get(self, issue_id, key).await
    }
    async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError> {
        FileStorage::delete(self, issue_id, key).await
    }
}

/// Disk-backed file storage for issue attachments.
/// Layout: <root>/<issue_id>/<attachment_id> — one directory per issue.
#[derive(Clone)]
pub struct FileStorage {
    root: PathBuf,
    max_upload_bytes: usize,
}

impl FileStorage {
    pub fn new(config: &StorageConfig) -> Self {
        Self {
            root: PathBuf::from(&config.dir),
            max_upload_bytes: config.max_upload_bytes,
        }
    }

    pub fn max_upload_bytes(&self) -> usize {
        self.max_upload_bytes
    }

    fn path_for(&self, issue_id: &str, key: &str) -> PathBuf {
        // Defense in depth: strip any path separators from the key.
        let safe_key = key.replace(['/', '\\'], "_");
        let safe_issue = issue_id.replace(['/', '\\'], "_");
        self.root.join(safe_issue).join(safe_key)
    }

    pub async fn put(&self, issue_id: &str, key: &str, bytes: Vec<u8>) -> Result<String, AppError> {
        if bytes.len() > self.max_upload_bytes {
            return Err(AppError::invalid_input(format!(
                "file exceeds the {} byte upload limit",
                self.max_upload_bytes
            )));
        }
        if bytes.is_empty() {
            return Err(AppError::invalid_input("file is empty"));
        }
        let path = self.path_for(issue_id, key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::internal(format!("storage mkdir failed: {e}")))?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|e| AppError::internal(format!("storage write failed: {e}")))?;
        Ok(key.to_string())
    }

    pub async fn get(&self, issue_id: &str, key: &str) -> Result<Vec<u8>, AppError> {
        let path = self.path_for(issue_id, key);
        tokio::fs::read(&path)
            .await
            .map_err(|_| AppError::not_found("attachment file", key))
    }

    pub async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError> {
        let path = self.path_for(issue_id, key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                self.remove_empty_issue_dir(&path).await;
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.remove_empty_issue_dir(&path).await;
                Ok(())
            }
            Err(e) => Err(AppError::internal(format!("storage delete failed: {e}"))),
        }
    }

    async fn remove_empty_issue_dir(&self, path: &std::path::Path) {
        let Some(parent) = path.parent() else {
            return;
        };
        match tokio::fs::remove_dir(parent).await {
            Ok(()) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(e) => tracing::warn!(
                path = %parent.display(),
                error = %e,
                "failed to remove empty issue storage directory"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_storage() -> (FileStorage, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("tasktracker-storage-{}", uuid::Uuid::new_v4()));
        let storage = FileStorage::new(&StorageConfig {
            dir: root.to_string_lossy().into_owned(),
            max_upload_bytes: 1024,
        });
        (storage, root)
    }

    #[tokio::test]
    async fn delete_removes_empty_issue_directory() {
        let (storage, root) = temp_storage();
        storage
            .put("issue-1", "attachment.txt", b"payload".to_vec())
            .await
            .unwrap();
        let issue_dir = root.join("issue-1");
        assert!(issue_dir.is_dir());

        storage.delete("issue-1", "attachment.txt").await.unwrap();

        assert!(!issue_dir.exists());
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
