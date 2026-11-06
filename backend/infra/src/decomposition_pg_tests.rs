use super::*;
use domain::decomposition::*;

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn decomposition_upgrade_preserves_existing_task_and_history() {
    let migrations = migration::Migrator::migrations();
    let before_plan = migrations
        .iter()
        .position(|migration| migration.name() == "m20261103_0000113_decomposition")
        .expect("published decomposition migration");
    let f = Fixture::new_with_migration_steps(Some(before_plan as u32)).await;
    let task = f.draft("existing-before-plan-storage", None).await;
    let before = f.snapshot().await;
    let issue =
        f.db.query_one(statement(
            "SELECT to_jsonb(i) AS original FROM issues i WHERE id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Json>("", "original")
        .unwrap();
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(f.snapshot().await, before);
    let after =
        f.db.query_one(statement(
            "SELECT to_jsonb(i) AS original FROM issues i WHERE id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Json>("", "original")
        .unwrap();
    assert_eq!(after, issue);
    assert!(
        f.repo
            .read_plan(task, &f.owner)
            .await
            .unwrap()
            .plan
            .is_none()
    );
}

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn decomposition_persistence_cas_history_acl_and_retry() {
    let f = Fixture::new().await;
    let task = f.draft("plan", None).await;
    let mut command = SavePlan {
        expected_revision: 0,
        idempotency_key: "create-plan".into(),
        reason: String::new(),
        fence: None,
        content: PlanContent {
            overview: "Use existing preferences".into(),
            source: None,
            sections: vec![
                PlanSection {
                    id: Uuid::new_v4(),
                    title: "Storage".into(),
                    description: "Persist preferences in the profile".into(),
                    criteria: "Read after write; deny another user".into(),
                    requirements: "R1,R2".into(),
                },
                PlanSection {
                    id: Uuid::new_v4(),
                    title: "Form".into(),
                    description: "Edit existing form".into(),
                    criteria: "Preserve input after failure".into(),
                    requirements: "R1".into(),
                },
            ],
        },
    };
    assert!(
        f.repo
            .read_plan(task, &f.owner)
            .await
            .unwrap()
            .plan
            .is_none()
    );
    let (first, replayed) = f
        .repo
        .save_plan(task, &f.owner, command.clone())
        .await
        .unwrap();
    assert!(!replayed);
    assert!(first.changed);
    assert_eq!(first.result.revision, 1);
    // Lost ACK: reissue the exact operation, and independently read its result.
    let (again, replayed) = f
        .repo
        .save_plan(task, &f.owner, command.clone())
        .await
        .unwrap();
    assert!(replayed);
    assert_eq!(json!(again), json!(first));
    assert_eq!(
        json!(
            f.repo
                .plan_operation(task, &f.owner, "create-plan")
                .await
                .unwrap()
        ),
        json!(first)
    );
    let mut bad = command.clone();
    bad.content.overview.push_str(" different");
    assert!(f.repo.save_plan(task, &f.owner, bad).await.is_err());
    let outsider = actor("foreign", true, &[]);
    assert!(f.repo.read_plan(task, &outsider).await.is_err());
    assert!(f.repo.plan_revision(task, &outsider, 1).await.is_err());
    assert!(
        f.repo
            .save_plan(task, &outsider, command.clone())
            .await
            .is_err()
    );
    let mut machine = actor("pm", false, &["task-tracker:read", "task-tracker:write"]);
    assert!(
        f.repo.read_plan(task, &machine).await.is_err(),
        "ordinary machine membership is not assignment authority"
    );
    // Save without content changes creates no new revision and needs no reason.
    command.expected_revision = 1;
    command.idempotency_key = "noop".into();
    let (noop, _) = f
        .repo
        .save_plan(task, &f.owner, command.clone())
        .await
        .unwrap();
    assert!(!noop.changed);
    assert_eq!(noop.result.revision, 1);
    // Rename and reorder preserve section identities; a substantive edit needs a reason.
    command.content.sections.swap(0, 1);
    command.content.sections[0].title = "Preferences form".into();
    command.idempotency_key = "edit".into();
    assert!(
        f.repo
            .save_plan(task, &f.owner, command.clone())
            .await
            .is_err()
    );
    command.reason = "Clarify form and order".into();
    // Two writers against revision 1: exactly one commit.
    let mut competing = command.clone();
    competing.idempotency_key = "competing".into();
    competing.content.overview.push_str(" alternative");
    let (left, right) = tokio::join!(
        f.repo.save_plan(task, &f.owner, command.clone()),
        f.repo.save_plan(task, &f.owner, competing)
    );
    assert!(left.is_ok() ^ right.is_ok());
    let second = f
        .repo
        .read_plan(task, &f.owner)
        .await
        .unwrap()
        .plan
        .unwrap();
    assert_eq!(second.revision, 2);
    let history = f
        .repo
        .plan_history(
            task,
            &f.owner,
            PlanHistoryQuery {
                limit: Some(1),
                before: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(history.revisions.len(), 1);
    assert_eq!(history.next_before, Some(2));
    let tail = f
        .repo
        .plan_history(
            task,
            &f.owner,
            PlanHistoryQuery {
                limit: Some(1),
                before: history.next_before,
            },
        )
        .await
        .unwrap();
    assert_eq!(tail.revisions[0].revision, 1);
    assert!(tail.next_before.is_none());
    assert_eq!(
        json!(f.repo.plan_revision(task, &f.owner, 1).await.unwrap()),
        json!(first.result)
    );
    // Replay stays the original accepted result after newer edits.
    assert_eq!(
        f.repo
            .plan_operation(task, &f.owner, "create-plan")
            .await
            .unwrap()
            .result
            .revision,
        1
    );
    let removed = second.content.sections[0].clone();
    command.expected_revision = 2;
    command.idempotency_key = "remove".into();
    command.content = second.content;
    command.content.sections.remove(0);
    f.repo
        .save_plan(task, &f.owner, command.clone())
        .await
        .unwrap();
    command.expected_revision = 3;
    command.idempotency_key = "reuse".into();
    command.content.sections.push(removed);
    assert!(
        f.repo
            .save_plan(task, &f.owner, command.clone())
            .await
            .is_err()
    );
    command.content.sections.last_mut().unwrap().id = Uuid::new_v4();
    command.idempotency_key = "new-section".into();
    f.repo
        .save_plan(task, &f.owner, command.clone())
        .await
        .unwrap();
    // Fresh human member can edit; revocation removes reads, writes and receipts.
    machine.trusted_human = true;
    let read = f.repo.read_plan(task, &machine).await.unwrap();
    assert!(read.can_edit);
    command.expected_revision = 4;
    command.idempotency_key = "member-edit".into();
    command.content.overview.push_str(" reviewed");
    f.repo
        .save_plan(task, &machine, command.clone())
        .await
        .unwrap();
    exec(&f.db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='pm')",vec![f.project.into()]).await.unwrap();
    assert!(f.repo.read_plan(task, &machine).await.is_err());
    assert!(f.repo.save_plan(task, &machine, command).await.is_err());
    assert!(
        f.repo
            .plan_operation(task, &machine, "member-edit")
            .await
            .is_err()
    );
    // History is immutable in PostgreSQL, not merely in the HTTP handler.
    assert!(
        f.db.execute(statement(
            "UPDATE issue_plan_revisions SET payload=payload WHERE issue_id=$1",
            vec![task.into()]
        ))
        .await
        .is_err()
    );
    assert!(
        f.db.execute(statement(
            "DELETE FROM issue_plan_revisions WHERE issue_id=$1",
            vec![task.into()]
        ))
        .await
        .is_err()
    );
    // Restart/reconnect reads the same durable head.
    let repo = PostgresSdlcRepository::connect(
        &std::env::var("TT_CANDIDATES_TEST_DATABASE_URL").unwrap(),
        f.repo.config.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        repo.read_plan(task, &f.owner)
            .await
            .unwrap()
            .plan
            .unwrap()
            .revision,
        5
    );
    // HTTP DTOs/statuses/no-store on the same PostgreSQL data. The actor here is
    // an explicit synthetic trusted fixture; central token verification is not mocked as live proof.
    let mut ctx = app::AppContext::new(
        Arc::new(shared::AppConfig::default()),
        Arc::new(domain::Repositories::default()),
        Arc::new(domain::InMemoryStorage::default()),
    );
    ctx.sdlc = Some(app::sdlc::SdlcService {
        repository: Arc::new(repo),
        fleet_configuration: None,
        configured_workflow: None,
    });
    let router = axum::Router::new()
        .route(
            "/issues/{id}/decomposition",
            axum::routing::get(api::routes::decomposition::read)
                .post(api::routes::decomposition::save),
        )
        .route(
            "/issues/{id}/decomposition/revisions",
            axum::routing::get(api::routes::decomposition::history),
        )
        .route(
            "/issues/{id}/decomposition/revisions/{revision}",
            axum::routing::get(api::routes::decomposition::revision),
        )
        .route(
            "/issues/{id}/decomposition/operations/{key}",
            axum::routing::get(api::routes::decomposition::operation),
        )
        .layer(axum::Extension(f.owner.clone()))
        .with_state(Arc::new(ctx));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let url = format!("http://{address}/issues/{task}/decomposition");
    for suffix in [
        "",
        "/revisions?limit=1",
        "/revisions/1",
        "/operations/create-plan",
    ] {
        let response = client.get(format!("{url}{suffix}")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.json::<Json>().await.unwrap().is_object());
    }
    let response = client
        .post(&url)
        .json(&SavePlan {
            expected_revision: 0,
            idempotency_key: "http-stale".into(),
            reason: "Stale edit".into(),
            fence: None,
            content: first.result.content,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    assert_eq!(
        client
            .get(format!("{url}/operations/unknown"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{url}/revisions?limit=0"))
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    let read: PlanReadback = client.get(&url).send().await.unwrap().json().await.unwrap();
    let current = read.plan.unwrap();
    let mut content = current.content;
    content.overview.push_str("; verified through HTTP");
    let command = SavePlan {
        expected_revision: current.revision,
        idempotency_key: "http-save".into(),
        reason: "Verify HTTP persistence".into(),
        content,
        fence: None,
    };
    let response = client.post(&url).json(&command).send().await.unwrap();
    assert_eq!(response.status(), 201);
    let saved: PlanOperation = response.json().await.unwrap();
    assert_eq!(saved.result.revision, 6);
    let replay = client.post(&url).json(&command).send().await.unwrap();
    assert_eq!(replay.status(), 200);
    assert_eq!(
        json!(replay.json::<PlanOperation>().await.unwrap()),
        json!(saved)
    );
    server.abort();
}
