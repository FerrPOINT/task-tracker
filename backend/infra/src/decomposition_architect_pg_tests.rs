//! Real PostgreSQL transactions with synthetic role/terminal fixtures; no live
//! agent execution or acceptance of the original user's business task.
use super::*;
use domain::decomposition::*;
use domain::sdlc_source::TaskSourceRepository;

/// Explicit synthetic task/assignment fixtures for a disposable system test. This
/// prepares no live workflow, owner approval or terminal acceptance evidence.
#[tokio::test]
#[ignore = "disposable PostgreSQL and ARCHITECT_QA_FIXTURE_SPEC/OUTPUT required"]
async fn seed_architect_system_fixture() {
    let input = std::fs::read(std::env::var("ARCHITECT_QA_FIXTURE_SPEC").unwrap()).unwrap();
    let spec: Json = serde_json::from_slice(&input).unwrap();
    let owner = Uuid::parse_str(spec["owner_subject"].as_str().unwrap()).unwrap();
    assert!(!owner.is_nil());
    let (f, queued) = ReworkFixture::with_architect_owner_inputs(
        Some(serde_json::from_value(spec["verification"].clone()).unwrap()),
        Some(serde_json::from_value(spec["requirements"].clone()).unwrap()),
        &owner.to_string(),
    )
    .await;
    let analyst = f
        .step(
            &queued,
            DeliveryResult::Analyst {
                analysis: serde_json::from_value(spec["analysis"].clone()).unwrap(),
            },
            0,
        )
        .await;
    let architect = &f.producers["architect"];
    let reserved = f
        .repo
        .lifecycle_command(
            f.task,
            &f.scheduler,
            LifecycleCommand::Reserve {
                expected_task_version: analyst.completed.task.version,
                machine_subject: Some(architect.subject.clone()),
                idempotency_key: "architect-system:reserve".into(),
            },
        )
        .await
        .unwrap()
        .0;
    let running = f
        .repo
        .lifecycle_command(
            f.task,
            &f.scheduler,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&reserved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: architect.subject.clone(),
                    dispatch_receipt: artifact("synthetic-system-test-dispatch"),
                },
                idempotency_key: "architect-system:bind".into(),
            },
        )
        .await
        .unwrap()
        .0;
    let context = f.repo.context(f.task, architect).await.unwrap();
    let result = json!({"fixture_only":true,"instance_id":f.repo.config.instance_id,
        "task_id":f.task,"project_id":context.project_id,"architect_subject":architect.subject,
        "reader_subject":f.producers["developer"].subject,"context":context,"running":running});
    std::fs::write(
        std::env::var("ARCHITECT_QA_FIXTURE_OUTPUT").unwrap(),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending _lifecycle_test"]
async fn decomposition_architect_requires_current_role_fence_and_requirements() {
    let (f, queued) = ReworkFixture::new().await;
    let analyst = f
        .step(
            &queued,
            DeliveryResult::Analyst {
                analysis: artifact("plan-analysis"),
            },
            0,
        )
        .await;
    let queued = analyst.completed;
    let architect = &f.producers["architect"];
    let reserved = f
        .repo
        .lifecycle_command(
            f.task,
            &f.scheduler,
            LifecycleCommand::Reserve {
                expected_task_version: queued.task.version,
                machine_subject: Some(architect.subject.clone()),
                idempotency_key: "plan:reserve".into(),
            },
        )
        .await
        .unwrap()
        .0;
    let assignment = reserved.assignment.as_ref().unwrap();
    assert!(!f.repo.read_plan(f.task, &f.owner).await.unwrap().can_edit);
    assert!(!f.repo.read_plan(f.task, architect).await.unwrap().can_edit);
    let running = f
        .repo
        .lifecycle_command(
            f.task,
            &f.scheduler,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&reserved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: architect.subject.clone(),
                    dispatch_receipt: artifact("plan-dispatch"),
                },
                idempotency_key: "plan:bind".into(),
            },
        )
        .await
        .unwrap()
        .0;
    let source = f
        .repo
        .read_plan(f.task, architect)
        .await
        .unwrap()
        .current_source
        .unwrap();
    let command = SavePlan {
        expected_revision: 0,
        idempotency_key: "architect:plan".into(),
        reason: String::new(),
        fence: Some(MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            assignment_version: assignment.fencing_token,
        }),
        content: PlanContent {
            overview: "Use existing profile and API".into(),
            source: Some(PlanSource {
                analysis_result_id: assignment.previous_result_id,
                ..source
            }),
            sections: vec![PlanSection {
                id: Uuid::new_v4(),
                title: "Preferences".into(),
                description: "Persist preferences in the existing profile".into(),
                criteria: "Unit and PostgreSQL tests cover save, retry and foreign user".into(),
                requirements: "R1".into(),
            }],
        },
    };
    assert!(
        f.repo
            .save_plan(f.task, &f.owner, command.clone())
            .await
            .is_err(),
        "human edits are locked during a mutating assignment"
    );
    assert!(
        f.repo
            .save_plan(f.task, &f.producers["developer"], command.clone())
            .await
            .is_err()
    );
    let mut wrong = command.clone();
    wrong.fence.as_mut().unwrap().assignment_version += 1;
    assert!(f.repo.save_plan(f.task, architect, wrong).await.is_err());
    let mut wrong = command.clone();
    wrong.content.source.as_mut().unwrap().requirements_hash = "b".repeat(64);
    assert!(f.repo.save_plan(f.task, architect, wrong).await.is_err());
    let mut wrong = command.clone();
    wrong.content.source.as_mut().unwrap().analysis_result_id = Some(Uuid::new_v4());
    assert!(f.repo.save_plan(f.task, architect, wrong).await.is_err());
    let (saved, replayed) = f
        .repo
        .save_plan(f.task, architect, command.clone())
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(saved.result.content, command.content);
    assert!(
        f.repo
            .save_plan(f.task, architect, command.clone())
            .await
            .unwrap()
            .1
    );
    let context = serde_json::to_value(f.repo.context(f.task, architect).await.unwrap()).unwrap();
    assert_eq!(
        context["previous_result_id"],
        json!(assignment.previous_result_id)
    );
    assert_eq!(
        context["selected_source_policy"]["source_revision"],
        json!(1)
    );
    assert_eq!(
        context["selected_source_policy"]["source"]["base_commit"],
        json!("a".repeat(40))
    );
    let selected: domain::sdlc_source::ProjectSourcePolicy =
        serde_json::from_value(context["selected_source_policy"].clone()).unwrap();
    let mut updated_source = selected.source.clone();
    updated_source.base_commit = "b".repeat(40);
    let mut verification = selected.verification.clone();
    verification.base_commit = updated_source.base_commit.clone();
    verification.observed_at = chrono::Utc::now();
    f.repo
        .set_source_policy(
            selected.project_id,
            &f.owner,
            domain::sdlc_source::SetSourcePolicy {
                expected_source_revision: Some(selected.source_revision),
                source: updated_source,
                idempotency_key: "plan:new-project-source".into(),
            },
            verification,
        )
        .await
        .unwrap();
    let context = serde_json::to_value(f.repo.context(f.task, architect).await.unwrap()).unwrap();
    assert_eq!(
        context["selected_source_policy"],
        json!(selected),
        "current project policy never replaces the task selection"
    );
    for field in [
        "assignment_id",
        "execution_id",
        "agent_id",
        "assignment_version",
    ] {
        let mut value = json!(command);
        value["idempotency_key"] = json!(format!("wrong:{field}"));
        value["fence"][field] = if field == "assignment_version" {
            json!(assignment.fencing_token + 1)
        } else {
            json!(Uuid::new_v4())
        };
        assert!(
            f.repo
                .save_plan(f.task, architect, serde_json::from_value(value).unwrap())
                .await
                .is_err()
        );
    }
    crate::sdlc::configured_test_support::delivery_comment(
        &f.repo,
        f.task,
        architect,
        assignment,
        "plan:final-comment",
    )
    .await;
    let reference = format!("/api/v1/issues/{}/decomposition/revisions/1", f.task);
    let mut proposal = SubmitDeliveryProposal {
        fence: delivery_fence(&running),
        workflow_receipt: artifact("synthetic-plan-handoff-fixture"),
        result: DeliveryResult::Architect {
            design: ArtifactReceipt {
                reference: reference.clone(),
                sha256: saved.result.content_sha256.clone(),
            },
        },
        idempotency_key: "plan:proposal".into(),
    };
    for invalid in [
        artifact("legacy-text-must-not-replace-plan"),
        ArtifactReceipt {
            reference: format!(
                "/api/v1/issues/{}/decomposition/revisions/1",
                Uuid::new_v4()
            ),
            sha256: saved.result.content_sha256.clone(),
        },
        ArtifactReceipt {
            reference: format!("/api/v1/issues/{}/decomposition/revisions/9", f.task),
            sha256: saved.result.content_sha256.clone(),
        },
        ArtifactReceipt {
            reference: reference.clone(),
            sha256: "b".repeat(64),
        },
    ] {
        let mut invalid_command = proposal.clone();
        invalid_command.result = DeliveryResult::Architect { design: invalid };
        assert!(
            f.repo
                .submit_delivery_proposal(f.task, architect, invalid_command)
                .await
                .is_err()
        );
    }
    let (accepted, _) = f
        .repo
        .submit_delivery_proposal(f.task, architect, proposal.clone())
        .await
        .unwrap();
    let mut newer = command;
    newer.expected_revision = 1;
    newer.idempotency_key = "plan:second".into();
    newer.reason = "Clarify the expected checks".into();
    newer.content.sections[0]
        .criteria
        .push_str(" and reconnect");
    f.repo.save_plan(f.task, architect, newer).await.unwrap();
    assert_eq!(
        json!(
            f.repo
                .submit_delivery_proposal(f.task, architect, proposal.clone())
                .await
                .unwrap()
                .0
        ),
        json!(accepted),
        "replay retains the exact submitted plan revision"
    );
    proposal.idempotency_key = "plan:another-proposal".into();
    assert!(
        f.repo
            .submit_delivery_proposal(f.task, architect, proposal)
            .await
            .is_err()
    );
    assert_eq!(
        f.repo
            .lifecycle_read(f.task, &f.reader)
            .await
            .unwrap()
            .receipt,
        running,
        "saving a plan never advances task SDLC"
    );
}
