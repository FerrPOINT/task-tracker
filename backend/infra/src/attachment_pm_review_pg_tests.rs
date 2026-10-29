//! Invoked by the existing ignored current-PM fixture, after the real claim (known producer dependency).
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) async fn unlisted(
    f: &Fixture,
    r: &Running,
    storage: &dyn domain::FileStorage,
) -> shared::AttachmentId {
    crate::sdlc::attachment_artifacts::publish(
        &f.db,
        &f.repo.config,
        &r.producer,
        shared::IssueId::from_uuid(r.task),
        domain::attachment_artifact::ArtifactUpload {
            file_name: "unlisted-evidence.json".into(),
            content_type: "application/json".into(),
            bytes: b"{\"synthetic\":\"not in saved blocker\"}".to_vec(),
        },
        storage,
    )
    .await
    .unwrap()
    .id
}

#[derive(Clone, Copy)]
enum DuringIo {
    None,
    OwnerRevoked,
    ProjectRevoked,
    CorruptBytes,
}

struct ObservedStorage<'a> {
    storage: &'a dyn domain::FileStorage,
    f: &'a Fixture,
    pm_subject: &'a str,
    action: DuringIo,
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl domain::FileStorage for ObservedStorage<'_> {
    async fn put(&self, _: &str, _: &str, _: Vec<u8>) -> Result<(), AppError> {
        Err(AppError::Forbidden)
    }
    async fn delete(&self, _: &str, _: &str) -> Result<(), AppError> {
        Err(AppError::Forbidden)
    }
    async fn get(&self, issue: &str, key: &str) -> Result<Vec<u8>, AppError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut bytes = self.storage.get(issue, key).await?;
        let write = async {
            match self.action {
                DuringIo::OwnerRevoked => {
                    exec(
                        &self.f.db,
                        "UPDATE users SET is_active=false WHERE central_sub='owner'",
                        vec![],
                    )
                    .await?
                }
                DuringIo::ProjectRevoked => {
                    exec(
                        &self.f.db,
                        "DELETE FROM project_members WHERE project_id=$1
                    AND user_id=(SELECT id FROM users WHERE central_sub=$2)",
                        vec![self.f.project.into(), self.pm_subject.into()],
                    )
                    .await?
                }
                DuringIo::CorruptBytes => bytes.push(b' '),
                DuringIo::None => {}
            }
            Ok::<(), AppError>(())
        };
        // This also proves the reader did not retain ACL/task locks across FileStorage I/O.
        tokio::time::timeout(std::time::Duration::from_secs(5), write)
            .await
            .expect("synthetic revocation must not wait for reader locks")?;
        Ok(bytes)
    }
}

pub(super) async fn check(
    f: &Fixture,
    r: &Running,
    pm: &Principal,
    old_pm: &Principal,
    unlisted: shared::AttachmentId,
    storage: &dyn domain::FileStorage,
) {
    let id: shared::AttachmentId = r
        .evidence
        .receipt
        .reference
        .strip_prefix("tracker-attachment:")
        .unwrap()
        .parse()
        .unwrap();
    let observed = ObservedStorage {
        storage,
        f,
        pm_subject: &pm.subject,
        action: DuringIo::None,
        calls: AtomicUsize::new(0),
    };
    let before = read_snapshot(f, r.task).await;
    let (item, bytes) =
        crate::sdlc::attachment_artifacts::download(&f.db, &f.repo.config, pm, id, &observed)
            .await
            .unwrap();
    assert_eq!(observed.calls.load(Ordering::SeqCst), 1);
    assert!(
        crate::sdlc::attachment_artifacts::download(&f.db, &f.repo.config, pm, unlisted, &observed)
            .await
            .is_err()
    );
    assert_eq!(observed.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        Seal::parse(item.storage_key.as_ref())
            .unwrap()
            .unwrap()
            .artifact(),
        r.evidence
    );
    app::attachment_artifacts::verify_bytes(&item, &bytes).unwrap();
    assert_eq!(read_snapshot(f, r.task).await, before);
    for denied in [old_pm, &f.reader] {
        assert!(
            crate::sdlc::critical_blockers::pm_evidence::download(
                &f.db,
                &f.repo.config,
                denied,
                id,
                &observed
            )
            .await
            .is_err()
        );
        assert_eq!(observed.calls.load(Ordering::SeqCst), 1);
    }
    assert!(
        crate::sdlc::attachment_artifacts::download(
            &f.db,
            &f.repo.config,
            pm,
            shared::AttachmentId::new(),
            &observed
        )
        .await
        .is_err()
    );
    assert_eq!(observed.calls.load(Ordering::SeqCst), 1);

    let ordinary = Uuid::new_v4();
    exec(&f.db,"INSERT INTO attachments(id,issue_id,author_id,file_name,content_type,size_bytes,storage_key,created_at)
        SELECT $1,issue_id,author_id,'ordinary.txt','text/plain',1,'ordinary-private-file',created_at FROM attachments WHERE id=$2",
        vec![ordinary.into(),id.as_uuid().into()]).await.unwrap();
    assert!(
        crate::sdlc::attachment_artifacts::download(
            &f.db,
            &f.repo.config,
            pm,
            shared::AttachmentId::from_uuid(ordinary),
            &observed
        )
        .await
        .is_err()
    );
    assert_eq!(observed.calls.load(Ordering::SeqCst), 1);
    exec(
        &f.db,
        "DELETE FROM attachments WHERE id=$1",
        vec![ordinary.into()],
    )
    .await
    .unwrap();

    for action in [
        DuringIo::OwnerRevoked,
        DuringIo::ProjectRevoked,
        DuringIo::CorruptBytes,
    ] {
        let observed = ObservedStorage {
            storage,
            f,
            pm_subject: &pm.subject,
            action,
            calls: AtomicUsize::new(0),
        };
        assert!(
            crate::sdlc::attachment_artifacts::download(&f.db, &f.repo.config, pm, id, &observed)
                .await
                .is_err()
        );
        assert_eq!(observed.calls.load(Ordering::SeqCst), 1);
        match observed.action {
            DuringIo::OwnerRevoked => exec(
                &f.db,
                "UPDATE users SET is_active=true WHERE central_sub='owner'",
                vec![],
            )
            .await
            .unwrap(),
            DuringIo::ProjectRevoked => exec(
                &f.db,
                "INSERT INTO project_members(project_id,user_id,role)
                SELECT $1,id,'developer' FROM users WHERE central_sub=$2",
                vec![f.project.into(), pm.subject.clone().into()],
            )
            .await
            .unwrap(),
            _ => {}
        }
        assert_eq!(read_snapshot(f, r.task).await, before);
    }
}
