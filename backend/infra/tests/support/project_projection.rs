//! Real HTTP transport and a native Tracker database; the responder models Wiki ACK loss.
use domain::project_spaces::ReplaceProjectSpaces;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use shared::{AppError, ProjectId, UserId, project_spaces::*, space_access::*};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
pub async fn verify(
    db: &DatabaseConnection,
    project: ProjectId,
    user: UserId,
    a: &SpaceRef,
    b: &SpaceRef,
    previous: &ConfirmedProjectSpaces,
) -> ConfirmedProjectSpaces {
    db.execute_unprepared("UPDATE tracker_space_catalog SET state='active' WHERE space_id='00000000-0000-4000-8000-000000000011'").await.unwrap();
    let add = ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: previous.revision,
        spaces: vec![a.clone(), b.clone()],
    };
    let added = infra::project_spaces::replace(
        db,
        project,
        &add,
        &super::permit(user, &[a.clone(), b.clone()], 60),
    )
    .await
    .unwrap();
    let remove = ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: added.revision,
        spaces: vec![b.clone()],
    };
    let latest = infra::project_spaces::replace(
        db,
        project,
        &remove,
        &super::permit(user, &[b.clone()], 60),
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}/", listener.local_addr().unwrap());
    let file = std::env::temp_dir().join(format!("project-projection-{}", Uuid::new_v4()));
    std::fs::write(&file, "synthetic-project-owner-credential").unwrap();
    // This one fresh native fixture owns its process/config; no live env or user secret is used.
    unsafe {
        std::env::set_var("TT_SPACES__WIKI_ORIGIN", origin);
        std::env::set_var("TT_SPACES__WIKI_TOKEN_FILE", &file);
    }
    let expected_project = latest.project.clone();
    let server = tokio::spawn(async move {
        let mut seen = Vec::<ProjectSpacesProjection>::new();
        for index in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let split;
            loop {
                if let Some(at) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    split = at + 4;
                    break;
                }
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0 && bytes.len() + n <= 1024 * 1024);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8(bytes[..split].to_vec()).unwrap();
            assert!(head.starts_with(&format!(
                "PUT /api/v2/projects/{}/{}/spaces/projection HTTP/1.1",
                expected_project.tracker_instance_id, expected_project.project_id
            )));
            let fields: Vec<_> = head.lines().filter_map(|s| s.split_once(':')).collect();
            let auth = fields
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
                .unwrap()
                .1
                .trim();
            assert_eq!(auth, "Bearer synthetic-project-owner-credential");
            let length: usize = fields
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .unwrap()
                .1
                .trim()
                .parse()
                .unwrap();
            assert!(length <= 1024 * 1024);
            while bytes.len() - split < length {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let command: ProjectSpacesProjection =
                serde_json::from_slice(&bytes[split..split + length]).unwrap();
            assert!(command.valid());
            seen.push(command.clone());
            if index == 0 {
                continue;
            } // Wiki applied the command, but the ACK was lost.
            let body =
                serde_json::to_vec(&ProjectSpacesProjectionReadback::confirmed(&command).unwrap())
                    .unwrap();
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        }
        seen
    });
    let pending =
        infra::project_space_outbox::reconcile(db, project, &super::permit(user, &[b.clone()], 60))
            .await
            .unwrap();
    assert!(!pending.confirmed);
    assert_eq!(
        pending.last_error.as_deref(),
        Some("wiki_projection_not_confirmed")
    );
    let completed =
        infra::project_space_outbox::reconcile(db, project, &super::permit(user, &[b.clone()], 60))
            .await
            .unwrap();
    assert!(completed.confirmed);
    assert_eq!(completed.revision, latest.revision);
    let requests = server.await.unwrap();
    assert_eq!(
        requests[0], requests[1],
        "lost ACK retries the original immutable command"
    );
    assert!(
        requests[1].revision < requests[2].revision,
        "later canonical revision waits for earlier ACK"
    );
    assert_eq!(requests[2].spaces, vec![b.clone()]);
    assert!(matches!(
        infra::project_space_outbox::reconcile(
            db,
            project,
            &super::permit(UserId::new(), &[b.clone()], 60)
        )
        .await,
        Err(AppError::Forbidden)
    ));
    std::fs::remove_file(file).unwrap();
    latest
}
