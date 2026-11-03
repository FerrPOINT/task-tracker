//! Real PostgreSQL transactions with synthetic role/terminal fixtures; no live
//! agent execution or acceptance of the original user's business task.
use super::*;
use domain::decomposition::*;

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
            .save_plan(f.task, architect, command)
            .await
            .unwrap()
            .1
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
