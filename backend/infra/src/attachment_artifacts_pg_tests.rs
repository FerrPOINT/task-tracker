//! Register as a child of the existing candidates PG fixture; no new runner/database setup.
//! All Fleet/native/Workflow terminal observations here are explicitly synthetic.
use super::*;
use crate::sdlc::attachment_artifacts as artifacts;
use domain::attachment_artifact::{ArtifactUpload, Seal};

fn upload(bytes: &[u8], name: &str) -> ArtifactUpload {
    ArtifactUpload {
        file_name: name.into(),
        content_type: "application/json".into(),
        bytes: bytes.to_vec(),
    }
}
fn delivery_fence(receipt: &LifecycleReceipt) -> DeliveryFence {
    let assignment = receipt.assignment.as_ref().unwrap();
    DeliveryFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        fencing_token: assignment.fencing_token,
        lease_id: assignment.lease_id,
        expected_lease_version: receipt.lease.as_ref().unwrap().version,
    }
}

async fn bind(f: &Fixture, task: Uuid, key: &str) -> LifecycleReceipt {
    let receipt = f
        .repo
        .lifecycle_read(task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let (reserved, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Reserve {
                expected_task_version: receipt.task.version,
                idempotency_key: format!("reserve-{key}"),
            },
        )
        .await
        .unwrap();
    let (bound, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&reserved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: "artifact-agent".into(),
                    dispatch_receipt: ArtifactReceipt {
                        reference: "fixture://synthetic-fleet-dispatch".into(),
                        sha256: "c".repeat(64),
                    },
                },
                idempotency_key: format!("bind-{key}"),
            },
        )
        .await
        .unwrap();
    bound
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; synthetic runtime observations"]
async fn actual_role_artifacts_are_fenced_replayed_predecessor_scoped_and_immutable() {
    let f = Fixture::new().await;
    let agent_id = Uuid::new_v4();
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,'artifact-agent@example.test','artifact-agent','artifact-agent','!','artifact-agent',true)",vec![agent_id.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![f.project.into(), agent_id.into()],
    )
    .await
    .unwrap();
    let task = f.confirmed("artifact-task", true).await;
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::EnsureQueued {
                idempotency_key: "queue".into(),
            },
        )
        .await
        .unwrap();
    let bound = bind(&f, task, "analyst").await;
    assert_eq!(bound.lease.as_ref().unwrap().holder_subject, "scheduler");
    let assignment = bound.assignment.as_ref().unwrap();
    let producer = actor(
        "artifact-agent",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &assignment.scope(),
        ],
    );
    let storage = domain::InMemoryStorage::default();
    let issue = shared::IssueId::from_uuid(task);
    let config = &f.repo.config;
    let bytes = b"{ \"actual\": \"analysis\" }";
    let (one, two) = tokio::join!(
        artifacts::publish(
            &f.db,
            config,
            &producer,
            issue,
            upload(bytes, "analysis.json"),
            &storage
        ),
        artifacts::publish(
            &f.db,
            config,
            &producer,
            issue,
            upload(bytes, "analysis.json"),
            &storage
        )
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.id, two.id);
    assert_eq!(one.created_at, two.created_at);
    assert_eq!(one.author_id.as_uuid(), agent_id);
    let seal = Seal::parse(one.storage_key.as_ref()).unwrap().unwrap();
    assert_eq!(seal.run_id, bound.binding.as_ref().unwrap().run_id);
    let (_, actual) = artifacts::download(&f.db, config, &producer, one.id, &storage)
        .await
        .unwrap();
    assert_eq!(actual, bytes);
    assert!(matches!(
        artifacts::download(&f.db, config, &f.reader, one.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let foreign = actor(
        "foreign",
        false,
        &["task-tracker:read", &assignment.scope()],
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &foreign, one.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let owner_impersonation = actor(
        "owner",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &assignment.scope(),
        ],
    );
    assert!(matches!(
        artifacts::publish(
            &f.db,
            config,
            &owner_impersonation,
            issue,
            upload(bytes, "owner.json"),
            &storage
        )
        .await,
        Err(AppError::Forbidden)
    ));
    let wrong_fence = actor(
        "artifact-agent",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &format!(
                "task-tracker:sdlc:delivery:{}:{}:{}:{}:{}",
                task,
                assignment.assignment_id,
                assignment.execution_id,
                assignment.agent_id,
                assignment.fencing_token + 1
            ),
        ],
    );
    assert!(matches!(
        artifacts::publish(
            &f.db,
            config,
            &wrong_fence,
            issue,
            upload(bytes, "wrong-fence.json"),
            &storage
        )
        .await,
        Err(AppError::Forbidden)
    ));
    let original = one.storage_key.to_string();
    exec(
        &f.db,
        "UPDATE attachments SET size_bytes=size_bytes+1 WHERE id=$1",
        vec![one.id.as_uuid().into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        artifacts::download(&f.db, config, &producer, one.id, &storage).await,
        Err(AppError::Conflict(_))
    ));
    exec(
        &f.db,
        "UPDATE attachments SET size_bytes=size_bytes-1 WHERE id=$1",
        vec![one.id.as_uuid().into()],
    )
    .await
    .unwrap();
    assert!(
        domain::FileStorage::delete(&storage, &task.to_string(), &original)
            .await
            .is_err()
    );
    let omitted = artifacts::publish(
        &f.db,
        config,
        &producer,
        issue,
        upload(b"{}", "unaccepted.json"),
        &storage,
    )
    .await
    .unwrap();
    let result = DeliveryResult::Analyst {
        analysis: seal.artifact().receipt,
    };
    let workflow_receipt = ArtifactReceipt {
        reference: "fixture://synthetic-workflow-terminal".into(),
        sha256: "d".repeat(64),
    };
    crate::sdlc::discussion::test_delivery_comment(&f.repo, task).await;
    let (proposal, _) = f
        .repo
        .submit_delivery_proposal(
            task,
            &producer,
            SubmitDeliveryProposal {
                fence: delivery_fence(&bound),
                workflow_receipt: workflow_receipt.clone(),
                result: result.clone(),
                idempotency_key: "proposal".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        artifacts::download(&f.db, config, &f.reader, one.id, &storage)
            .await
            .unwrap()
            .1,
        bytes
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &f.reader, omitted.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let binding = bound.binding.as_ref().unwrap();
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Complete {
                fence: delivery_fence(&bound),
                result,
                terminal: TerminalReceipt {
                    chat_id: binding.chat_id,
                    run_id: binding.run_id,
                    workflow_receipt,
                    fleet_receipt: ArtifactReceipt {
                        reference: "fixture://synthetic-native-terminal".into(),
                        sha256: "e".repeat(64),
                    },
                    output_sha256: proposal.output_sha256,
                },
                idempotency_key: "complete".into(),
            },
        )
        .await
        .unwrap();
    let architect = bind(&f, task, "architect").await;
    let consumer = actor(
        "artifact-agent",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &architect.assignment.as_ref().unwrap().scope(),
        ],
    );
    assert_eq!(
        artifacts::download(&f.db, config, &consumer, one.id, &storage)
            .await
            .unwrap()
            .1,
        bytes
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &consumer, omitted.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(
        artifacts::list(&f.db, config, &consumer, issue)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        artifacts::publish(
            &f.db,
            config,
            &producer,
            issue,
            upload(bytes, "analysis.json"),
            &storage
        )
        .await
        .is_err()
    );
    // Actual database clock, no trigger disabling or protected ledger edits.
    f.db.execute_unprepared("SELECT pg_sleep(31)")
        .await
        .unwrap();
    assert!(matches!(
        artifacts::publish(
            &f.db,
            config,
            &consumer,
            issue,
            upload(b"{}", "expired.json"),
            &storage
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    exec(
        &f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![f.project.into(), agent_id.into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        artifacts::download(&f.db, config, &consumer, one.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let owner_id: Uuid =
        f.db.query_one(statement(
            "SELECT id FROM users WHERE central_sub='owner'",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    let foreign_id: Uuid =
        f.db.query_one(statement(
            "SELECT id FROM users WHERE central_sub='foreign'",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    // Transfer the existing fixture connection, never clone it or create another pool.
    let attachments = crate::repos::SeaOrmRepositories::new(f.db).attachments;
    attachments
        .authorize_sealed_human_read(issue, shared::UserId::from_uuid(owner_id))
        .await
        .unwrap();
    assert!(matches!(
        attachments
            .authorize_sealed_human_read(issue, shared::UserId::from_uuid(foreign_id))
            .await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        attachments.delete(one.id).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(attachments.get_by_id(one.id).await.unwrap().id, one.id);
}
