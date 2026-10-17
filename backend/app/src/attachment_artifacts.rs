use domain::attachment_artifact::{Seal, is_sealed};
use domain::sdlc::Principal;
use sha2::{Digest, Sha256};
use shared::AppError;

pub fn raw_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn verify_bytes(attachment: &domain::Attachment, bytes: &[u8]) -> Result<(), AppError> {
    let seal = Seal::parse(attachment.storage_key.as_ref())?.ok_or(AppError::Forbidden)?;
    seal.validate_attachment(attachment)?;
    if attachment.size_bytes != bytes.len() as i64 || seal.sha256 != raw_sha256(bytes) {
        return Err(AppError::conflict("sealed artifact checksum mismatch"));
    }
    Ok(())
}

pub fn deny_sealed_delete(key: &str) -> Result<(), AppError> {
    if is_sealed(key) {
        return Err(AppError::conflict("sealed role artifact is immutable"));
    }
    Ok(())
}

/// The download path has no task identifier. Metadata authorization is mandatory after dispatch.
pub fn delivery_attachment_route(
    actor: &Principal,
    method: &str,
    path: &str,
) -> Result<bool, AppError> {
    let Some(grant) = crate::sdlc_lifecycle::delivery_grant(actor)? else {
        return Ok(false);
    };
    let task_route = format!("/api/v1/issues/{}/attachments", grant.task_id);
    let download = is_download_path(path);
    let write = method == "POST" && path == task_route;
    let read = method == "GET" && (path == task_route || download);
    if !write && !read {
        return Err(AppError::Forbidden);
    }
    crate::sdlc_lifecycle::producer(actor, write)?;
    Ok(true)
}

pub fn is_download_path(path: &str) -> bool {
    path.strip_prefix("/api/v1/attachments/")
        .and_then(|part| part.strip_suffix("/download"))
        .is_some_and(|id| crate::sdlc::canonical_pm_uuid(id).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_routes_are_canonical_exact_and_method_scoped() {
        let task = uuid::Uuid::new_v4();
        let mut actor = Principal {
            subject: "actual-role".into(),
            human_session: false,
            trusted_human: false,
            scopes: [
                format!(
                    "task-tracker:sdlc:delivery:{task}:{}:{}:{}:1",
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4(),
                    uuid::Uuid::new_v4()
                ),
                "task-tracker:read".into(),
            ]
            .into(),
        };
        let path = format!("/api/v1/issues/{task}/attachments");
        assert!(delivery_attachment_route(&actor, "GET", &path).unwrap());
        assert!(delivery_attachment_route(&actor, "POST", &path).is_err());
        actor.scopes.insert("task-tracker:write".into());
        assert!(delivery_attachment_route(&actor, "POST", &path).unwrap());
        let download = format!("/api/v1/attachments/{}/download", uuid::Uuid::new_v4());
        assert!(delivery_attachment_route(&actor, "GET", &download).unwrap());
        for path in [
            format!("{path}/"),
            format!("/api/v1/issues/{}/attachments", uuid::Uuid::new_v4()),
            download.to_uppercase(),
            format!("{download}/"),
            "/api/v1/attachments/not-uuid/download".into(),
            format!("/api/v1/issues/{task}"),
        ] {
            assert!(delivery_attachment_route(&actor, "GET", &path).is_err());
        }
        for method in ["HEAD", "PUT", "PATCH", "DELETE"] {
            assert!(delivery_attachment_route(&actor, method, &download).is_err());
        }
        actor.human_session = true;
        assert!(delivery_attachment_route(&actor, "GET", &download).is_err());
    }
    #[test]
    fn checksum_is_of_raw_bytes_not_reserialized_json() {
        assert_ne!(raw_sha256(b"{\"a\":1}"), raw_sha256(b"{ \"a\": 1 }"));
        assert!(deny_sealed_delete("sdlc-artifact-v1_malformed").is_err());
        assert!(deny_sealed_delete("legacy-report").is_ok());
    }
    #[test]
    fn raw_bytes_metadata_and_foreign_identity_are_checked() {
        use domain::attachment_artifact::{ArtifactBinding, Seal};
        use uuid::Uuid;
        let task = Uuid::new_v4();
        let bytes = b"{ \"analysis\": true }";
        let seal = Seal::new(
            task,
            ArtifactBinding {
                assignment_id: Uuid::new_v4(),
                execution_id: Uuid::new_v4(),
                role: domain::sdlc_lifecycle::DeliveryRole::Analyst,
            },
            Uuid::new_v4(),
            "analysis.json",
            "application/json",
            raw_sha256(bytes),
        )
        .unwrap();
        let mut item = domain::Attachment {
            id: shared::AttachmentId::from_uuid(seal.artifact_id),
            issue_id: shared::IssueId::from_uuid(task),
            author_id: shared::UserId::new(),
            file_name: "analysis.json".into(),
            content_type: "application/json".into(),
            size_bytes: bytes.len() as i64,
            storage_key: seal.key().into(),
            created_at: shared::now(),
        };
        verify_bytes(&item, bytes).unwrap();
        assert!(verify_bytes(&item, b"{\"analysis\":true}").is_err());
        item.size_bytes += 1;
        assert!(verify_bytes(&item, bytes).is_err());
        item.size_bytes -= 1;
        item.issue_id = shared::IssueId::new();
        assert!(verify_bytes(&item, bytes).is_err());
    }
}
