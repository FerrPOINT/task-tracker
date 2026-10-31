use domain::{
    sdlc_lifecycle::{LifecycleStage as Stage, LifecycleStatus as Status},
    task_lifecycle::{Action, TransitionCommand},
};
use infra::repos::SeaOrmRepositories;
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use shared::{IssueId, UserId};
use uuid::Uuid;

#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn task_lifecycle_commands_replay_and_serialize_competing_human_transitions() {
    let url =
        std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").expect("owned test database required");
    let db = Database::connect(url.as_str()).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/task_lifecycle_fixture.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.unwrap();
    let mut listener = sea_orm::sqlx::postgres::PgListener::connect(url.as_str())
        .await
        .unwrap();
    listener.listen("pdlc_task_lifecycle").await.unwrap();
    let repo = SeaOrmRepositories::new(Database::connect(url.as_str()).await.unwrap()).issues;
    let task = IssueId::from_uuid(Uuid::parse_str("50000000-0000-4000-8000-000000000001").unwrap());
    let actor = UserId::from_uuid(Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap());
    let project = shared::ProjectId::from_uuid(
        Uuid::parse_str("20000000-0000-4000-8000-000000000001").unwrap(),
    );
    let legacy_todo = shared::StatusId::from_uuid(Uuid::from_u128(1));
    assert_eq!(repo.count_backlog(project, legacy_todo).await.unwrap(), 1);
    let page = repo
        .list_backlog_page(project, legacy_todo, 1, 0)
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, task);
    assert!(
        repo.list_backlog_page(project, legacy_todo, 1, 1)
            .await
            .unwrap()
            .is_empty()
    );
    let initial = repo.task_lifecycle(task, true).await.unwrap();
    assert_eq!(
        (
            initial.state.stage,
            initial.state.status,
            initial.state.revision
        ),
        (Stage::Backlog, Status::Ready, 1)
    );
    let sprint = Uuid::parse_str("60000000-0000-4000-8000-000000000001").unwrap();
    db.execute_unprepared("INSERT INTO sprints(id,project_id,name,state) VALUES('60000000-0000-4000-8000-000000000001','20000000-0000-4000-8000-000000000001','Preserved future sprint','future'); UPDATE issues SET sprint_id='60000000-0000-4000-8000-000000000001' WHERE key='CORE-1'").await.unwrap();
    let command = TransitionCommand {
        action: Action::Start,
        expected_revision: 1,
        idempotency_key: "start-one".into(),
    };
    let first = repo
        .transition_task_lifecycle(task, actor, &command)
        .await
        .unwrap();
    assert_eq!(
        (first.state.stage, first.state.status, first.state.revision),
        (Stage::Backlog, Status::InProgress, 2)
    );
    assert_eq!(
        repo.transition_task_lifecycle(task, actor, &command)
            .await
            .unwrap()
            .state,
        first.state
    );
    // This shared projection is also called by the agent writer: changing state
    // must not silently remove the issue from planning or distort sprint reports.
    assert_eq!(
        repo.get_by_id(task)
            .await
            .unwrap()
            .sprint_id
            .map(|id| id.as_uuid()),
        Some(sprint)
    );
    let changed = TransitionCommand {
        action: Action::Complete,
        expected_revision: 2,
        idempotency_key: "start-one".into(),
    };
    assert!(
        repo.transition_task_lifecycle(task, actor, &changed)
            .await
            .is_err()
    );
    let stale = TransitionCommand {
        action: Action::Complete,
        expected_revision: 1,
        idempotency_key: "stale".into(),
    };
    assert!(
        repo.transition_task_lifecycle(task, actor, &stale)
            .await
            .is_err()
    );
    let a = TransitionCommand {
        action: Action::Complete,
        expected_revision: 2,
        idempotency_key: "race-a".into(),
    };
    let b = TransitionCommand {
        action: Action::Complete,
        expected_revision: 2,
        idempotency_key: "race-b".into(),
    };
    let (a, b) = tokio::join!(
        repo.transition_task_lifecycle(task, actor, &a),
        repo.transition_task_lifecycle(task, actor, &b)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let final_state = repo.task_lifecycle(task, true).await.unwrap().state;
    assert_eq!(
        (final_state.stage, final_state.status, final_state.revision),
        (Stage::Analysis, Status::Ready, 3)
    );
    for _ in 0..2 {
        let notice = tokio::time::timeout(std::time::Duration::from_secs(2), listener.recv())
            .await
            .unwrap()
            .unwrap();
        let event: shared::TrackerEvent = serde_json::from_str(notice.payload()).unwrap();
        assert_eq!(event.project_key(), "CORE");
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), listener.recv())
            .await
            .is_err()
    );
    let counts=db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT (SELECT count(*) FROM task_lifecycle_history WHERE task_id='50000000-0000-4000-8000-000000000001') AS history,(SELECT count(*) FROM task_lifecycle_events WHERE task_id='50000000-0000-4000-8000-000000000001') AS events,(SELECT count(*) FROM sdlc_pm_executions) AS executions".to_owned())).await.unwrap().unwrap();
    assert_eq!(counts.try_get::<i64>("", "history").unwrap(), 3);
    assert_eq!(counts.try_get::<i64>("", "executions").unwrap(), 0);
    assert_eq!(counts.try_get::<i64>("", "events").unwrap(), 3);
    let mut state = final_state;
    for stage in [
        Stage::Analysis,
        Stage::Architecture,
        Stage::Development,
        Stage::Review,
        Stage::Testing,
        Stage::Deployment,
    ] {
        assert_eq!(state.stage, stage);
        for action in [Action::Start, Action::Complete] {
            let command = TransitionCommand {
                action,
                expected_revision: state.revision,
                idempotency_key: format!("walk-{}-{action:?}", state.revision),
            };
            if stage == Stage::Analysis && action == Action::Complete {
                db.execute_unprepared("UPDATE issues SET description='' WHERE key='CORE-1'")
                    .await
                    .unwrap();
                assert!(
                    repo.transition_task_lifecycle(task, actor, &command)
                        .await
                        .is_err()
                );
                assert_eq!(repo.task_lifecycle(task, true).await.unwrap().state, state);
                db.execute_unprepared("UPDATE issues SET description='Unicode: кириллица' WHERE key='CORE-1'; INSERT INTO task_lifecycle_board_limits(project_id,stage,wip_limit) VALUES('20000000-0000-4000-8000-000000000001','Architecture',0)").await.unwrap();
                assert!(
                    repo.transition_task_lifecycle(task, actor, &command)
                        .await
                        .is_err()
                );
                assert_eq!(repo.task_lifecycle(task, true).await.unwrap().state, state);
                db.execute_unprepared(
                    "UPDATE task_lifecycle_board_limits SET wip_limit=1 WHERE stage='Architecture'",
                )
                .await
                .unwrap();
            }
            state = repo
                .transition_task_lifecycle(task, actor, &command)
                .await
                .unwrap()
                .state;
        }
    }
    assert_eq!(
        (state.stage, state.status),
        (Stage::Deployment, Status::Completed)
    );
    state = repo
        .transition_task_lifecycle(
            task,
            actor,
            &TransitionCommand {
                action: Action::Reopen,
                expected_revision: state.revision,
                idempotency_key: "reopen-final".into(),
            },
        )
        .await
        .unwrap()
        .state;
    assert_eq!((state.stage, state.status), (Stage::Rework, Status::Ready));
    // Core board content, status projection and action revision must share one snapshot.
    let snapshot = repo.task_stage_board_snapshot(project, true).await.unwrap();
    assert_eq!(snapshot.columns.len(), 9);
    assert_eq!(snapshot.cards.len(), 4);
    assert!(snapshot.sprint.is_none());
    for card in &snapshot.cards {
        assert_eq!(
            card.issue.status_id,
            domain::task_lifecycle::projection_status_id(
                card.lifecycle.state.stage,
                card.lifecycle.state.status
            )
        );
        assert_eq!(card.reporter_name.as_deref(), Some("Operator"));
    }
    let concurrent_command = TransitionCommand {
        action: Action::Start,
        expected_revision: state.revision,
        idempotency_key: "snapshot-race".into(),
    };
    let (snapshot, changed) = tokio::join!(
        repo.task_stage_board_snapshot(project, true),
        repo.transition_task_lifecycle(task, actor, &concurrent_command)
    );
    let changed = changed.unwrap();
    let snapshot = snapshot.unwrap();
    let card = snapshot
        .cards
        .iter()
        .find(|card| card.issue.id == task)
        .unwrap();
    assert!([state.revision, changed.state.revision].contains(&card.lifecycle.state.revision));
    assert_eq!(
        card.issue.status_id,
        domain::task_lifecycle::projection_status_id(
            card.lifecycle.state.stage,
            card.lifecycle.state.status
        )
    );
    // Return to Ready through a genuine domain operation for the filter fixtures below.
    let done = repo
        .transition_task_lifecycle(
            task,
            actor,
            &TransitionCommand {
                action: Action::Complete,
                expected_revision: changed.state.revision,
                idempotency_key: "snapshot-race-complete".into(),
            },
        )
        .await
        .unwrap();
    state = repo
        .transition_task_lifecycle(
            task,
            actor,
            &TransitionCommand {
                action: Action::RequestRework,
                expected_revision: done.state.revision,
                idempotency_key: "filter-rework".into(),
            },
        )
        .await
        .unwrap()
        .state;
    // A lifecycle status spans multiple stages. Apply both predicates before pagination.
    let all = repo
        .task_lifecycle_states(&page.iter().map(|i| i.id).collect::<Vec<_>>())
        .await
        .unwrap();
    assert_eq!(all[&task], state);
    let query = domain::IssueQuery {
        project_id: Some(project),
        lifecycle_status: Some(Status::Ready),
        sort_by: Some("created".into()),
        sort_order: Some("asc".into()),
        limit: 1,
        ..Default::default()
    };
    let first = repo.list(query.clone()).await.unwrap();
    let second = repo
        .list(domain::IssueQuery {
            offset: 1,
            ..query.clone()
        })
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].id, second[0].id);
    assert!(
        repo.list(domain::IssueQuery {
            offset: 2,
            ..query.clone()
        })
        .await
        .unwrap()
        .is_empty()
    );
    let staged = repo
        .list(domain::IssueQuery {
            lifecycle_stage: Some(Stage::Rework),
            ..query.clone()
        })
        .await
        .unwrap();
    assert_eq!(staged.iter().map(|i| i.id).collect::<Vec<_>>(), vec![task]);
    let expr = domain::jql::parse("project = CORE").unwrap();
    assert_eq!(
        repo.list(domain::IssueQuery {
            jql: Some(expr),
            lifecycle_stage: Some(Stage::Rework),
            ..query
        })
        .await
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect::<Vec<_>>(),
        vec![task]
    );

    assert_eq!(
        db.query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM sdlc_delivery_results".to_owned()
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap(),
        0
    );
    // Mandatory core state must not make ordinary trash/restore/admin purge unusable.
    let disposable =
        IssueId::from_uuid(Uuid::parse_str("50000000-0000-4000-8000-000000000002").unwrap());
    assert!(repo.purge(disposable).await.is_err());
    repo.delete(disposable).await.unwrap();
    repo.restore(disposable).await.unwrap();
    repo.delete(disposable).await.unwrap();
    repo.purge(disposable).await.unwrap();
    assert!(repo.get_by_id_include_deleted(disposable).await.is_err());
    assert_eq!(repo.task_lifecycle(task, true).await.unwrap().state, state);
    let left=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM task_lifecycle_history WHERE task_id='50000000-0000-4000-8000-000000000002') AS history,(SELECT count(*) FROM task_lifecycle_purge_permits) AS permits".to_owned())).await.unwrap().unwrap();
    assert_eq!(left.try_get::<i64>("", "history").unwrap(), 0);
    assert_eq!(left.try_get::<i64>("", "permits").unwrap(), 0);
}

#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn historical_idle_task_transfers_to_manual_control_without_fabricating_execution() {
    let url = std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    let db = Database::connect(url.as_str()).await.unwrap();
    let identity = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        identity
            .try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(identity.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/task_lifecycle_fixture.sql"
    ))
    .await
    .unwrap();
    // A real historical business aggregate without an agent assignment is idle.
    // This fixture intentionally creates no completion, lease or deployment proof.
    db.execute_unprepared(r#"INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state)
        VALUES('50000000-0000-4000-8000-000000000001','40000000-0000-4000-8000-000000000001',
        '20000000-0000-4000-8000-000000000001','50000000-0000-4000-8000-000000000001',
        '10000000-0000-4000-8000-000000000001',
        '{"tracker_instance_id":"40000000-0000-4000-8000-000000000001","project_id":"20000000-0000-4000-8000-000000000001",
        "task_id":"50000000-0000-4000-8000-000000000001","root_task_id":"50000000-0000-4000-8000-000000000001",
        "owner_subject":"10000000-0000-4000-8000-000000000001","stage":"Analysis","status":"InProgress",
        "assignment":null,"confirmation_revision":null,"questions":[],"revisions":[],"confirmations":[],"evidence":[]}');
        INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state)
        SELECT '50000000-0000-4000-8000-000000000002',tracker_instance_id,project_id,
            '50000000-0000-4000-8000-000000000002',owner_subject,
            (state-'status')||jsonb_build_object('task_id','50000000-0000-4000-8000-000000000002',
                'root_task_id','50000000-0000-4000-8000-000000000002','stage','Clarification')
        FROM sdlc_tasks WHERE task_id='50000000-0000-4000-8000-000000000001';
        SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot());"#).await.unwrap();
    let task = IssueId::from_uuid(Uuid::parse_str("50000000-0000-4000-8000-000000000001").unwrap());
    let actor = UserId::from_uuid(Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap());
    let repo = SeaOrmRepositories::new(Database::connect(url.as_str()).await.unwrap()).issues;
    let clarification =
        IssueId::from_uuid(Uuid::parse_str("50000000-0000-4000-8000-000000000002").unwrap());
    let first_command = TransitionCommand {
        action: Action::Start,
        expected_revision: 1,
        idempotency_key: "clarification-start".into(),
    };
    let started = repo
        .transition_task_lifecycle(clarification, actor, &first_command)
        .await
        .unwrap();
    assert_eq!(started.state.stage, Stage::Draft);
    assert_eq!(
        started.draft_submode,
        Some(domain::sdlc::DraftSubmode::Clarification)
    );
    assert_eq!(
        repo.task_lifecycle(clarification, true)
            .await
            .unwrap()
            .draft_submode,
        started.draft_submode
    );
    let completed = repo
        .transition_task_lifecycle(
            clarification,
            actor,
            &TransitionCommand {
                action: Action::Complete,
                expected_revision: 2,
                idempotency_key: "clarification-complete".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (completed.state.stage, completed.state.status),
        (Stage::Backlog, Status::Ready)
    );
    assert!(completed.draft_submode.is_none());
    let original = repo
        .transition_task_lifecycle(clarification, actor, &first_command)
        .await
        .unwrap();
    assert_eq!(original.state, started.state);
    assert_eq!(original.draft_submode, started.draft_submode);
    assert!(original.allowed_actions.is_empty());
    let old = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT state FROM sdlc_tasks WHERE task_id='50000000-0000-4000-8000-000000000001'"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<serde_json::Value>("", "state")
        .unwrap();
    let initial = repo.task_lifecycle(task, true).await.unwrap();
    assert!(!initial.execution_busy);
    assert_eq!(initial.allowed_actions.len(), 1);
    // Ordinary SQL writers cannot flip custody or use a mismatched manual permit.
    assert!(db.execute_unprepared("UPDATE sdlc_tasks SET manual_control=true WHERE task_id='50000000-0000-4000-8000-000000000001'").await.is_err());
    let first = repo
        .transition_task_lifecycle(
            task,
            actor,
            &TransitionCommand {
                action: Action::Complete,
                expected_revision: 1,
                idempotency_key: "human-takeover".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (first.state.stage, first.state.status, first.state.revision),
        (Stage::Architecture, Status::Ready, 2)
    );
    let row=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT t.manual_control,h.previous_state,(SELECT count(*) FROM sdlc_pm_executions) AS runs,(SELECT count(*) FROM sdlc_delivery_results) AS results FROM sdlc_tasks t JOIN task_lifecycle_custody_history h ON h.task_id=t.task_id WHERE t.task_id='50000000-0000-4000-8000-000000000001'".to_owned())).await.unwrap().unwrap();
    assert!(row.try_get::<bool>("", "manual_control").unwrap());
    assert_eq!(
        row.try_get::<serde_json::Value>("", "previous_state")
            .unwrap(),
        old
    );
    assert_eq!(row.try_get::<i64>("", "runs").unwrap(), 0);
    assert_eq!(row.try_get::<i64>("", "results").unwrap(), 0);
    assert!(
        db.execute_unprepared("UPDATE task_lifecycle_custody_history SET previous_state='{}'")
            .await
            .is_err()
    );
    assert!(db.execute_unprepared("UPDATE sdlc_tasks SET state=jsonb_set(state,'{stage}','\"Development\"') WHERE task_id='50000000-0000-4000-8000-000000000001'").await.is_err());
    assert!(db.execute_unprepared("UPDATE sdlc_tasks SET manual_control=false WHERE task_id='50000000-0000-4000-8000-000000000001'").await.is_err());
    // The takeover does not leave a permanent old Analysis guard on later human work.
    let next = repo
        .transition_task_lifecycle(
            task,
            actor,
            &TransitionCommand {
                action: Action::Start,
                expected_revision: 2,
                idempotency_key: "architecture-start".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (next.state.stage, next.state.status),
        (Stage::Architecture, Status::InProgress)
    );
}

#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn fresh_model_bootstraps_without_execution_and_retains_its_identity() {
    let url = std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    let db = Database::connect(url.as_str()).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(
        infra::task_lifecycle_identity::is_pristine(&url)
            .await
            .unwrap()
    );
    assert!(
        infra::task_lifecycle_identity::prepare_bootstrap(&url)
            .await
            .unwrap()
    );
    // An interruption before migrations retains the same first-bootstrap identity.
    let bootstrap = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT bootstrap_id FROM task_model_bootstrap_provenance WHERE singleton".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "bootstrap_id")
        .unwrap();
    assert!(
        infra::task_lifecycle_identity::prepare_bootstrap(&url)
            .await
            .unwrap()
    );
    migration::Migrator::up(&db, None).await.unwrap();
    // Simulate process death after schema creation, before model activation.
    assert!(
        infra::task_lifecycle_identity::prepare_bootstrap(&url)
            .await
            .unwrap()
    );
    assert_eq!(
        db.query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT bootstrap_id FROM task_model_bootstrap_provenance WHERE singleton".to_owned()
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "bootstrap_id")
        .unwrap(),
        bootstrap
    );
    assert!(
        !infra::task_lifecycle_identity::is_pristine(&url)
            .await
            .unwrap()
    );
    infra::task_lifecycle_identity::ensure(&url, None, true)
        .await
        .unwrap();
    assert!(
        !infra::task_lifecycle_identity::prepare_bootstrap(&url)
            .await
            .unwrap()
    );
    let read = || {
        db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT instance_id,(SELECT enabled FROM task_lifecycle_control WHERE singleton) AS enabled FROM sdlc_instance WHERE singleton".to_owned()))
    };
    let first = read().await.unwrap().unwrap();
    let identity = first.try_get::<String>("", "instance_id").unwrap();
    assert!(!Uuid::parse_str(&identity).unwrap().is_nil());
    assert!(first.try_get::<bool>("", "enabled").unwrap());
    infra::task_lifecycle_identity::ensure(&url, None, false)
        .await
        .unwrap();
    assert!(
        infra::task_lifecycle_identity::ensure(&url, Some("different-installation"), false)
            .await
            .is_err()
    );
    assert_eq!(
        read()
            .await
            .unwrap()
            .unwrap()
            .try_get::<String>("", "instance_id")
            .unwrap(),
        identity
    );
    let fixture=include_str!("../../migration/src/task_lifecycle_fixture.sql").replace("INSERT INTO sdlc_instance(singleton,instance_id) VALUES(true,'40000000-0000-4000-8000-000000000001');","");
    let split = fixture.find("INSERT INTO issues(").unwrap();
    db.execute_unprepared(&fixture[..split]).await.unwrap();
    let project = shared::ProjectId::from_uuid(
        Uuid::parse_str("20000000-0000-4000-8000-000000000001").unwrap(),
    );
    let repo = SeaOrmRepositories::new(Database::connect(url.as_str()).await.unwrap()).issues;
    let empty = repo.task_stage_board_snapshot(project, true).await.unwrap();
    assert!(empty.cards.is_empty());
    assert_eq!(empty.columns.len(), 9);
    db.execute_unprepared(&fixture[split..]).await.unwrap();
    let rows = db
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT state FROM sdlc_tasks ORDER BY task_id".to_owned(),
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let state = row.try_get::<serde_json::Value>("", "state").unwrap();
        assert_eq!(state["stage"], "Draft");
        assert_eq!(state["status"], "Ready");
        assert!(state["assignment"].is_null());
        assert!(state.get("lifecycle").is_none());
    }
    let board = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT json_array_length(columns) AS n FROM boards".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(board.try_get::<i32>("", "n").unwrap(), 9);
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM sdlc_pm_executions".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 0);
}

#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn current_stage_handoff_preparation_is_owner_scoped_read_only_and_tracks_material_changes() {
    use domain::sdlc::{Principal, SdlcConfig, SdlcRepository};
    use domain::task_handoff::TaskExecutionBlocker as Blocker;
    let url = std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    let db = Database::connect(url.as_str()).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/task_lifecycle_fixture.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.unwrap();
    let repository = infra::sdlc::PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "40000000-0000-4000-8000-000000000001".into(),
            single_task_project_ids: Default::default(),
            reservation_scheduler_subject: "scheduler-qa".into(),
            orchestrator_subject: "orchestrator-qa".into(),
            verifier_subject: "verifier-qa".into(),
        },
    )
    .await
    .unwrap();
    let actor = Principal {
        subject: "10000000-0000-4000-8000-000000000001".into(),
        trusted_human: true,
        human_session: true,
        scopes: Default::default(),
    };
    let task = Uuid::parse_str("50000000-0000-4000-8000-000000000001").unwrap();
    let first = repository
        .prepare_current_stage_handoff(task, &actor)
        .await
        .unwrap();
    assert_eq!(first.state.stage, Stage::Backlog);
    assert_eq!(first.state.status, Status::Ready);
    assert_eq!(first.entry.unwrap().role.key(), "project_manager");
    assert_eq!(first.issue_sha256.len(), 64);
    assert_eq!(
        first.blockers,
        vec![
            Blocker::ExecutionNotConfigured,
            Blocker::RoutingNotConfigured,
            Blocker::SourceUnavailable
        ]
    );
    assert!(first.routing.is_none() && first.source.is_none());
    let repeated = repository
        .prepare_current_stage_handoff(task, &actor)
        .await
        .unwrap();
    assert_eq!(repeated.issue_sha256, first.issue_sha256);
    assert_eq!(repeated.state, first.state);
    let mut machine = actor.clone();
    machine.human_session = false;
    machine.trusted_human = false;
    assert!(
        repository
            .prepare_current_stage_handoff(task, &machine)
            .await
            .is_err()
    );
    db.execute_unprepared("UPDATE issues SET position=position+100 WHERE key='CORE-1'")
        .await
        .unwrap();
    assert_eq!(
        repository
            .prepare_current_stage_handoff(task, &actor)
            .await
            .unwrap()
            .issue_sha256,
        first.issue_sha256
    );
    db.execute_unprepared(
        "UPDATE issues SET description='Changed real handoff materials' WHERE key='CORE-1'",
    )
    .await
    .unwrap();
    let changed = repository
        .prepare_current_stage_handoff(task, &actor)
        .await
        .unwrap();
    assert_eq!(changed.state, first.state);
    assert_ne!(changed.issue_sha256, first.issue_sha256);
    let row=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM sdlc_pm_executions) AS runs,(SELECT count(*) FROM task_lifecycle_history) AS history".to_owned())).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "runs").unwrap(), 0);
    assert_eq!(row.try_get::<i64>("", "history").unwrap(), 4);
    // Imported ordinary tasks are manually owned. Retained history grants no run.
    let context = repository.context(task, &actor).await.unwrap();
    assert!(context.assignment.is_none());
    assert_eq!(context.waiting_reason.as_deref(), Some("manual_control"));
    assert!(repository.context(task, &machine).await.is_err());
    assert!(repository.read(task, &machine).await.is_err());
    assert!(repository.read(task, &actor).await.is_ok());
    let history = serde_json::to_value(repository.read(task, &actor).await.unwrap()).unwrap();
    let error = repository
        .execute(
            task,
            &actor,
            domain::sdlc::SdlcCommand::Confirm {
                revision: 1,
                command: domain::sdlc::ConfirmCommand {
                    content_hash: "a".repeat(64),
                    idempotency_key: "retired-human-confirm".into(),
                    expected_routing_policy_version: None,
                },
            },
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("task_lifecycle_handoff_required")
    );
    assert_eq!(
        serde_json::to_value(repository.read(task, &actor).await.unwrap()).unwrap(),
        history
    );
}

// Owner observations below are synthetic; this proves persistence, not a live launch.
#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn current_stage_human_materials_commit_atomically_replay_and_preserve_progress() {
    use app::task_handoff::CurrentStageHandoffRepository;
    use domain::sdlc::{Principal, SdlcConfig, SdlcRepository};
    use domain::sdlc_source::{
        SetSourcePolicy, SourcePolicySpec, TaskSourceRepository, VerifiedSource,
    };
    use domain::task_handoff::TaskHandoffCommand;
    use serde_json::json;
    let url = std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    let db = Database::connect(url.as_str()).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/task_lifecycle_fixture.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared("INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,description,reporter_id,priority,labels,position,time_spent_seconds) VALUES('50000000-0000-4000-8000-000000000005','20000000-0000-4000-8000-000000000001','CORE-5','Task','00000000-0000-0000-0000-000000000001','Legacy clarification','Preserved real inputs','10000000-0000-4000-8000-000000000001','Medium','[]',5,0); INSERT INTO sdlc_tasks(task_id,project_id,root_task_id,tracker_instance_id,owner_subject,state) VALUES('50000000-0000-4000-8000-000000000005','20000000-0000-4000-8000-000000000001','50000000-0000-4000-8000-000000000005','40000000-0000-4000-8000-000000000001','10000000-0000-4000-8000-000000000001','{\"task_id\":\"50000000-0000-4000-8000-000000000005\",\"root_task_id\":\"50000000-0000-4000-8000-000000000005\",\"project_id\":\"20000000-0000-4000-8000-000000000001\",\"tracker_instance_id\":\"40000000-0000-4000-8000-000000000001\",\"owner_subject\":\"10000000-0000-4000-8000-000000000001\",\"stage\":\"Clarification\",\"confirmation_revision\":null,\"assignment\":null,\"questions\":[],\"revisions\":[],\"confirmations\":[],\"evidence\":[]}')").await.unwrap();
    db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.unwrap();
    let project = Uuid::parse_str("20000000-0000-4000-8000-000000000001").unwrap();
    let repository = infra::sdlc::PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "40000000-0000-4000-8000-000000000001".into(),
            single_task_project_ids: [project].into_iter().collect(),
            reservation_scheduler_subject: "scheduler-qa".into(),
            orchestrator_subject: "orchestrator-qa".into(),
            verifier_subject: "verifier-qa".into(),
        },
    )
    .await
    .unwrap();
    let actor = Principal {
        subject: "10000000-0000-4000-8000-000000000001".into(),
        trusted_human: true,
        human_session: true,
        scopes: Default::default(),
    };
    let mut routes = json!({});
    for (index, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,"instruction_profile":"single-task-v1",
            "instruction_manifest_sha256":"b".repeat(64),"namespace_id":(index+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(index+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),"profile":format!("single-task-v1-{suffix}"),
            "workflow_catalog_version":1,"workflow_catalog_sha256":"c".repeat(64)});
    }
    repository
        .set_routing_policy(
            project,
            &actor,
            domain::sdlc_routing::SetRoutingPolicy {
                expected_version: None,
                routes: serde_json::from_value(routes).unwrap(),
                idempotency_key: "fixture-routing".into(),
            },
        )
        .await
        .unwrap();
    let namespace = shared::resource_context::NamespaceRef {
        registry_instance_id: Uuid::new_v4(),
        namespace_id: Uuid::new_v4(),
    };
    let binding = json!({"schema_version":1,"namespace":namespace,"resource":{"kind":"tracker_project","instance_id":Uuid::new_v4(),"resource_id":project},
        "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",
        [project.into(),namespace.registry_instance_id.into(),namespace.namespace_id.into(),binding.into()])).await.unwrap();
    let source = SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "d".repeat(40),
    };
    let verified = VerifiedSource {
        namespace,
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        public_name: "fixture/source".into(),
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: source.base_ref.clone(),
        protected_refs: vec![],
        observed_at: chrono::Utc::now(),
    };
    repository
        .set_source_policy(
            project,
            &actor,
            SetSourcePolicy {
                expected_source_revision: None,
                source,
                idempotency_key: "fixture-source".into(),
            },
            verified,
        )
        .await
        .unwrap();
    let command = |preview: &domain::task_handoff::PreparedCurrentStageHandoff,
                   key: &str|
     -> TaskHandoffCommand {
        serde_json::from_value(json!({
        "expected_revision":preview.state.revision,"expected_routing_policy_version":preview.routing.as_ref().unwrap().version,
        "confirmed_issue_sha256":preview.issue_sha256,"idempotency_key":key,"requirements":{"goal":"Actual human consent",
        "scope":["Current source"],"exclusions":[],"scenarios":["Retain current progress"],"acceptance_criteria":["Actual verified result"],
        "constraints":[],"dependencies":[],"assumptions":[],"checklist":["result-check"],"prerequisites":["source-input"]}})).unwrap()
    };
    let task = Uuid::parse_str("50000000-0000-4000-8000-000000000002").unwrap();
    let before = repository
        .prepare_current_stage_handoff(task, &actor)
        .await
        .unwrap();
    assert!(before.blockers.is_empty(), "{:?}", before.blockers);
    assert_eq!(
        (before.state.stage, before.state.status),
        (Stage::Development, Status::InProgress)
    );
    let stale = command(&before, "stale-materials");
    db.execute_unprepared(
        "UPDATE issues SET description='Human edited materials after preview' WHERE key='CORE-2'",
    )
    .await
    .unwrap();
    assert!(
        repository
            .save_handoff_materials(task, &actor, &stale)
            .await
            .is_err()
    );
    let preview = repository
        .prepare_current_stage_handoff(task, &actor)
        .await
        .unwrap();
    let original = command(&preview, "current-human-original");
    // Fail after the requirement and confirmation inserts, proving whole-tx rollback.
    db.execute_unprepared("CREATE FUNCTION injected_handoff_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$; CREATE TRIGGER injected_handoff_failure BEFORE INSERT ON task_handoff_materials FOR EACH ROW EXECUTE FUNCTION injected_handoff_failure()").await.unwrap();
    assert!(
        repository
            .save_handoff_materials(task, &actor, &original)
            .await
            .is_err()
    );
    db.execute_unprepared("DROP TRIGGER injected_handoff_failure ON task_handoff_materials; DROP FUNCTION injected_handoff_failure()").await.unwrap();
    let n=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM sdlc_requirements) AS requirements,(SELECT count(*) FROM sdlc_confirmations) AS confirmations".to_owned())).await.unwrap().unwrap();
    assert_eq!(n.try_get::<i64>("", "requirements").unwrap(), 0);
    assert_eq!(n.try_get::<i64>("", "confirmations").unwrap(), 0);
    let mut pat_actor = actor.clone();
    pat_actor.human_session = false;
    assert!(
        repository
            .prepare_current_stage_handoff(task, &pat_actor)
            .await
            .is_ok()
    );
    let (receipt, replayed) = repository
        .save_handoff_materials(task, &pat_actor, &original)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(receipt.entry.state, preview.state);
    assert_eq!(receipt.resulting_revision, preview.state.revision + 1);
    assert_eq!(receipt.entry.role.key(), "developer");
    let after = repository
        .prepare_current_stage_handoff(task, &actor)
        .await
        .unwrap();
    assert_eq!(
        (after.state.stage, after.state.status),
        (Stage::Development, Status::InProgress)
    );
    assert_eq!(after.state.revision, receipt.resulting_revision);
    let pending = repository
        .pending_handoff_materials(task, &actor)
        .await
        .unwrap();
    assert_eq!(pending.receipt.handoff_id, receipt.handoff_id);
    assert_eq!(pending.routing.snapshot_id, receipt.routing_snapshot_id);
    assert_eq!(pending.source.confirmation_id, receipt.confirmation_id);
    assert_eq!(
        pending.receipt.issue_materials_sha256,
        preview.issue_materials_sha256
    );
    // Content may change without changing core_revision. Such consent cannot launch.
    db.execute_unprepared(
        "UPDATE issues SET description='Changed after real consent' WHERE key='CORE-2'",
    )
    .await
    .unwrap();
    assert!(
        repository
            .pending_handoff_materials(task, &actor)
            .await
            .is_err()
    );
    assert!(
        repository
            .save_handoff_materials(task, &actor, &original)
            .await
            .unwrap()
            .1
    );
    db.execute_unprepared(
        "UPDATE issues SET description='Human edited materials after preview' WHERE key='CORE-2'",
    )
    .await
    .unwrap();
    assert!(
        repository
            .pending_handoff_materials(task, &actor)
            .await
            .is_ok()
    );
    let saved = repository.read(task, &actor).await.unwrap();
    assert!(saved.assignment.is_none());
    assert!(
        !saved.retained_fields.contains_key("flow")
            && !saved.retained_fields.contains_key("lifecycle")
    );
    assert_eq!(
        saved.confirmations.last().unwrap().stage,
        domain::sdlc::Stage::Development
    );
    assert_eq!(
        saved.revisions.last().unwrap().author_subject,
        actor.subject
    );
    assert!(
        repository
            .save_handoff_materials(task, &actor, &original)
            .await
            .unwrap()
            .1
    );
    let mut changed = original.clone();
    changed.requirements.goal = "Changed reused payload".into();
    assert!(
        repository
            .save_handoff_materials(task, &actor, &changed)
            .await
            .is_err()
    );
    let mut machine = actor.clone();
    machine.human_session = false;
    machine.trusted_human = false;
    assert!(
        repository
            .save_handoff_materials(task, &machine, &original)
            .await
            .is_err()
    );
    // A real human input cannot silently authorize a raw stage write or a legacy producer.
    assert!(db.execute_unprepared("UPDATE sdlc_tasks SET state=jsonb_set(state,'{stage}','\"Deployment\"'::jsonb) WHERE task_id='50000000-0000-4000-8000-000000000002'").await.is_err());
    let a = command(&after, "race-human-a");
    let b = command(&after, "race-human-b");
    let (a, b) = tokio::join!(
        repository.save_handoff_materials(task, &actor, &a),
        repository.save_handoff_materials(task, &actor, &b)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    // PM entry is genuine Backlog consent as well; it is not a reset to Draft.
    let pm_task = Uuid::parse_str("50000000-0000-4000-8000-000000000001").unwrap();
    let pm = repository
        .prepare_current_stage_handoff(pm_task, &actor)
        .await
        .unwrap();
    let mut pm_command = command(&pm, "human-backlog");
    pm_command.requirements.scope.clear();
    pm_command.requirements.scenarios.clear();
    pm_command.requirements.acceptance_criteria.clear();
    pm_command.requirements.checklist.clear();
    pm_command.requirements.prerequisites.clear();
    let (pm_receipt, _) = repository
        .save_handoff_materials(pm_task, &actor, &pm_command)
        .await
        .unwrap();
    assert_eq!(pm_receipt.entry.state.stage, Stage::Backlog);
    assert_eq!(pm_receipt.entry.role.key(), "project_manager");
    let n=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM task_handoff_materials) AS consents,(SELECT count(*) FROM sdlc_requirements) AS requirements,(SELECT count(*) FROM sdlc_confirmations) AS confirmations,(SELECT count(*) FROM sdlc_pm_executions) AS pm_runs,(SELECT count(*) FROM sdlc_delivery_assignments) AS assignments,(SELECT count(*) FROM task_handoff_material_write_permits) AS permits".to_owned())).await.unwrap().unwrap();
    assert_eq!(n.try_get::<i64>("", "consents").unwrap(), 3);
    assert_eq!(n.try_get::<i64>("", "requirements").unwrap(), 3);
    assert_eq!(n.try_get::<i64>("", "confirmations").unwrap(), 3);
    assert_eq!(n.try_get::<i64>("", "pm_runs").unwrap(), 0);
    assert_eq!(n.try_get::<i64>("", "assignments").unwrap(), 0);
    assert_eq!(n.try_get::<i64>("", "permits").unwrap(), 0);
    // Immutable audit size stays independent of the growing requirements history.
    // Full before/after aggregate snapshots are only transient transaction permits.
    let audit=db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT max(octet_length(to_jsonb(m)::text))-min(octet_length(to_jsonb(m)::text)) AS growth FROM task_handoff_materials m".to_owned())).await.unwrap().unwrap();
    assert!(audit.try_get::<i32>("", "growth").unwrap() < 1024);
    db.execute_unprepared("UPDATE issues SET deleted_at=clock_timestamp() WHERE key='CORE-2'")
        .await
        .unwrap();
    let error = db
        .execute_unprepared(
            "SELECT task_lifecycle_purge_manual_core('50000000-0000-4000-8000-000000000002')",
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("task_lifecycle_execution_history_retained")
    );
    db.execute_unprepared("UPDATE issues SET deleted_at=NULL WHERE key='CORE-2'")
        .await
        .unwrap();
    assert!(
        repository
            .pending_handoff_materials(task, &actor)
            .await
            .is_ok()
    );
    let legacy = Uuid::parse_str("50000000-0000-4000-8000-000000000005").unwrap();
    let legacy_preview = repository
        .prepare_current_stage_handoff(legacy, &actor)
        .await
        .unwrap();
    assert_eq!(
        (legacy_preview.state.stage, legacy_preview.state.status),
        (Stage::Draft, Status::Ready)
    );
    let (legacy_receipt, _) = repository
        .save_handoff_materials(
            legacy,
            &actor,
            &command(&legacy_preview, "human-clarification"),
        )
        .await
        .unwrap();
    assert_eq!(legacy_receipt.entry.state.stage, Stage::Draft);
    let retained = repository.read(legacy, &actor).await.unwrap();
    assert_eq!(retained.stage, domain::sdlc::Stage::Clarification);
    assert_eq!(retained.status, None);
    assert_eq!(
        retained.confirmations.last().unwrap().stage,
        domain::sdlc::Stage::Draft
    );
    assert!(
        repository
            .pending_handoff_materials(legacy, &actor)
            .await
            .is_ok()
    );
}

#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn existing_empty_model_database_does_not_gain_first_bootstrap_authority() {
    let url = std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    let db = Database::connect(url.as_str()).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    assert!(
        !infra::task_lifecycle_identity::prepare_bootstrap(&url)
            .await
            .unwrap()
    );
    assert!(
        infra::task_lifecycle_identity::ensure(&url, None, true)
            .await
            .is_err()
    );
    infra::task_lifecycle_identity::ensure(&url, None, false)
        .await
        .unwrap();
    let row=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT enabled FROM task_lifecycle_control WHERE singleton) AS enabled,(SELECT count(*) FROM task_model_bootstrap_provenance) AS provenance".to_owned())).await.unwrap().unwrap();
    assert!(!row.try_get::<bool>("", "enabled").unwrap());
    assert_eq!(row.try_get::<i64>("", "provenance").unwrap(), 0);
}
