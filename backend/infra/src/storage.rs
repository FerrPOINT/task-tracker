use shared::{AppError, StorageConfig};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        if let Some(seal) = domain::attachment_artifact::Seal::parse(key)? {
            if app::sdlc::canonical_pm_uuid(issue_id).is_none()
                || seal.sha256 != app::attachment_artifacts::raw_sha256(&bytes)
            {
                return Err(AppError::conflict("sealed artifact bytes or task invalid"));
            }
            return self.put_sealed(issue_id, key, &bytes).await;
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
        if let Some(seal) = domain::attachment_artifact::Seal::parse(key)? {
            let file = tokio::fs::File::open(&path)
                .await
                .map_err(|_| AppError::not_found("attachment file", key))?;
            let mut bytes = Vec::new();
            file.take(self.max_upload_bytes as u64 + 1)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| AppError::conflict("sealed artifact read failed"))?;
            if bytes.is_empty()
                || bytes.len() > self.max_upload_bytes
                || app::attachment_artifacts::raw_sha256(&bytes) != seal.sha256
            {
                return Err(AppError::conflict("sealed artifact checksum mismatch"));
            }
            return Ok(bytes);
        }
        tokio::fs::read(&path)
            .await
            .map_err(|_| AppError::not_found("attachment file", key))
    }

    pub async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError> {
        app::attachment_artifacts::deny_sealed_delete(key)?;
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

    async fn put_sealed(&self, issue: &str, key: &str, bytes: &[u8]) -> Result<String, AppError> {
        let path = self.path_for(issue, key);
        let parent = path
            .parent()
            .ok_or_else(|| AppError::internal("artifact path invalid"))?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| AppError::internal("artifact directory unavailable"))?;
        let temporary = parent.join(format!(".artifact-{}.tmp", uuid::Uuid::new_v4()));
        // Publish a complete inode atomically without replacing an existing sealed destination.
        let outcome = async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .await
                .map_err(|_| AppError::internal("artifact staging unavailable"))?;
            file.write_all(bytes)
                .await
                .map_err(|_| AppError::internal("artifact staging write failed"))?;
            file.sync_all()
                .await
                .map_err(|_| AppError::internal("artifact staging sync failed"))?;
            drop(file);
            match tokio::fs::hard_link(&temporary, &path).await {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(_) => return Err(AppError::internal("artifact immutable publication failed")),
            }
            let retained = self.get(issue, key).await?;
            if retained.as_slice() != bytes {
                return Err(AppError::conflict("artifact immutable replay differs"));
            }
            Ok(key.to_owned())
        }
        .await;
        // Only this invocation's staging file; never unlink the sealed destination on error.
        match tokio::fs::remove_file(&temporary).await {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => tracing::warn!("artifact staging cleanup failed"),
        }
        outcome
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

    #[tokio::test]
    async fn sealed_bytes_concurrent_replay_tamper_and_delete() {
        use domain::attachment_artifact::{ArtifactBinding, Seal};
        let (storage, root) = temp_storage();
        let issue = uuid::Uuid::new_v4().to_string();
        let bytes = b"{ \"actual\": true }".to_vec();
        let seal = Seal::new(
            issue.parse().unwrap(),
            ArtifactBinding {
                assignment_id: uuid::Uuid::new_v4(),
                execution_id: uuid::Uuid::new_v4(),
                role: domain::sdlc_lifecycle::DeliveryRole::Analyst,
            },
            uuid::Uuid::new_v4(),
            "analysis.json",
            "application/json",
            app::attachment_artifacts::raw_sha256(&bytes),
        )
        .unwrap();
        let key = seal.key();
        let (a, b) = tokio::join!(
            storage.put(&issue, &key, bytes.clone()),
            storage.put(&issue, &key, bytes.clone())
        );
        assert_eq!(a.unwrap(), key);
        assert_eq!(b.unwrap(), key);
        assert_eq!(storage.get(&issue, &key).await.unwrap(), bytes);
        assert!(
            storage
                .put(&issue, &key, b"different".to_vec())
                .await
                .is_err()
        );
        assert!(storage.delete(&issue, &key).await.is_err());
        let mut entries = tokio::fs::read_dir(root.join(&issue)).await.unwrap();
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            names.push(entry.file_name());
        }
        assert_eq!(names, vec![std::ffi::OsString::from(&key)]);
        tokio::fs::write(storage.path_for(&issue, &key), b"external-tamper")
            .await
            .unwrap();
        assert!(storage.get(&issue, &key).await.is_err());
        assert!(storage.put(&issue, &key, bytes).await.is_err());
        assert_eq!(
            tokio::fs::read(storage.path_for(&issue, &key))
                .await
                .unwrap(),
            b"external-tamper"
        );
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
