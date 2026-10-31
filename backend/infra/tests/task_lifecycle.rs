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
    let migrations = migration::Migrator::migrations();
    let before_routing_index = migrations
        .iter()
        .position(|m| m.name().ends_with("0000116_human_routing_inventory"))
        .unwrap();
    migration::Migrator::up(&db, Some(before_routing_index as u32))
        .await
        .unwrap();
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
            // New human entry uses real routing/source readiness, not the legacy project opt-in.
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
        namespace: namespace.clone(),
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
    // Work accounting is independent; real new discussion material invalidates consent.
    db.execute_unprepared(
        "UPDATE issues SET time_spent_seconds=time_spent_seconds+1 WHERE key='CORE-2'",
    )
    .await
    .unwrap();
    assert!(
        repository
            .pending_handoff_materials(task, &actor)
            .await
            .is_ok()
    );
    db.execute_unprepared("INSERT INTO comments(id,issue_id,author_id,body,created_at,updated_at) VALUES('70000000-0000-4000-8000-000000000001','50000000-0000-4000-8000-000000000002','10000000-0000-4000-8000-000000000001','New human input after consent',clock_timestamp(),clock_timestamp())").await.unwrap();
    assert!(
        repository
            .pending_handoff_materials(task, &actor)
            .await
            .is_err()
    );
    db.execute_unprepared("DELETE FROM comments WHERE id='70000000-0000-4000-8000-000000000001'")
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
    // Upgrade populated consent history without rewriting any receipt/payload or inventing native work.
    let before_index=db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY handoff_id) AS history FROM task_handoff_materials m".to_owned())).await.unwrap().unwrap().try_get::<serde_json::Value>("","history").unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    let after_index=db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY handoff_id) AS history FROM task_handoff_materials m".to_owned())).await.unwrap().unwrap().try_get::<serde_json::Value>("","history").unwrap();
    assert_eq!(before_index, after_index);
    let indexed=db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT (SELECT count(*) FROM task_handoff_materials) AS expected,(SELECT count(*) FROM task_handoff_routing_generations) AS actual,(SELECT count(*) FROM sdlc_task_routing_generations) AS reviews,(SELECT count(*) FROM sdlc_pm_executions) AS executions".to_owned())).await.unwrap().unwrap();
    assert!(indexed.try_get::<i64>("", "expected").unwrap() > 0);
    assert_eq!(
        indexed.try_get::<i64>("", "expected").unwrap(),
        indexed.try_get::<i64>("", "actual").unwrap()
    );
    assert_eq!(indexed.try_get::<i64>("", "reviews").unwrap(), 0);
    assert_eq!(indexed.try_get::<i64>("", "executions").unwrap(), 0);
    // Baseline status3 is Done, not Review: its agreed Deployment/Completed state
    // must not silently start a new agent or a real deployment.
    let completed = repository
        .prepare_current_stage_handoff(
            Uuid::parse_str("50000000-0000-4000-8000-000000000003").unwrap(),
            &actor,
        )
        .await
        .unwrap();
    assert_eq!(
        (completed.state.stage, completed.state.status),
        (Stage::Deployment, Status::Completed)
    );
    assert!(completed.entry.is_none());
    assert!(
        completed
            .blockers
            .contains(&domain::task_handoff::TaskExecutionBlocker::AlreadyCompleted)
    );
    // Metadata belongs to the already registered scheduler, never a human or
    // arbitrary read token. The old saved-only consents are not queued work.
    db.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash,central_sub) VALUES('10000000-0000-4000-8000-000000000003','scheduler@example.test','scheduler','Fixture scheduler','fixture','scheduler-qa'); INSERT INTO project_members(project_id,user_id,role) VALUES('20000000-0000-4000-8000-000000000001','10000000-0000-4000-8000-000000000003','member')").await.unwrap();
    let scheduler = Principal {
        subject: "scheduler-qa".into(),
        trusted_human: false,
        human_session: false,
        scopes: ["task-tracker:read".into()].into(),
    };
    let query = domain::sdlc_candidates::DeliveryCandidatesQuery {
        project_id: project,
        after: None,
        limit: 1,
    };
    assert!(
        repository
            .requested_handoffs(&actor, query.clone())
            .await
            .is_err()
    );
    assert!(
        repository
            .requested_handoffs(&scheduler, query.clone())
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        repository
            .requested_handoff_execution(task, &scheduler)
            .await
            .is_err()
    );
    // Queue ownership is durable, but is not a native running/assignment claim.
    let queued_task = Uuid::parse_str("50000000-0000-4000-8000-000000000004").unwrap();
    let queued_preview = repository
        .prepare_current_stage_handoff(queued_task, &actor)
        .await
        .unwrap();
    let queued_command = command(&queued_preview, "actual-durable-human-request");
    let entry = queued_preview.entry.unwrap();
    let mut observed_source = queued_preview.source.as_ref().unwrap().verification.clone();
    observed_source.observed_at = chrono::Utc::now();
    let configuration = app::sdlc_configuration::MatchedConfiguration {
        selection_sha256: app::sdlc_configuration::MatchedConfiguration::selection_hash(
            entry.route(queued_preview.routing.as_ref().unwrap()),
            entry.role.key(),
            entry.mode.key(),
        )
        .unwrap(),
        observation_ref: Uuid::new_v4(),
        observed_at: chrono::Utc::now(),
    };
    let observed = app::task_handoff_observations::MatchedHandoffInputs::bind(
        &queued_preview,
        configuration,
        observed_source,
    )
    .unwrap();
    // Corrupt only the audit deadline in a fixture trigger. The deferred commit
    // guard must roll the consent, history and request back without real sleeps.
    db.execute_unprepared("CREATE FUNCTION injected_expired_observation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.execution_requested THEN NEW.queue_observations=jsonb_set(NEW.queue_observations,'{valid_until}',to_jsonb((clock_timestamp()-interval '1 second')::text)); END IF; RETURN NEW; END $$; CREATE TRIGGER injected_expired_observation BEFORE INSERT ON task_handoff_materials FOR EACH ROW EXECUTE FUNCTION injected_expired_observation()").await.unwrap();
    let error = repository
        .request_current_stage_handoff(queued_task, &actor, &queued_command, &observed)
        .await
        .unwrap_err();
    assert!(matches!(error, shared::AppError::Conflict(_)));
    db.execute_unprepared("DROP TRIGGER injected_expired_observation ON task_handoff_materials; DROP FUNCTION injected_expired_observation()").await.unwrap();
    assert_eq!(
        repository
            .prepare_current_stage_handoff(queued_task, &actor)
            .await
            .unwrap()
            .state,
        queued_preview.state
    );
    let (requested, replayed) = repository
        .request_current_stage_handoff(queued_task, &pat_actor, &queued_command, &observed)
        .await
        .unwrap();
    assert!(!replayed);
    let source_link = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT snapshot,created_by FROM task_repository_links WHERE issue_id=$1",
            vec![queued_task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        source_link
            .try_get::<Uuid>("", "created_by")
            .unwrap()
            .to_string(),
        actor.subject
    );
    assert_eq!(
        requested.issue_materials_sha256,
        queued_preview.issue_materials_sha256
    );
    // A display caption is not a different repository. A real unrelated link is
    // still a material change and cannot be hidden by source association projection.
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "UPDATE task_repository_links SET snapshot=jsonb_set(snapshot,'{public_name}','\"Renamed source caption\"'::jsonb) WHERE issue_id=$1",vec![queued_task.into()])).await.unwrap();
    assert!(
        repository
            .pending_handoff_materials(queued_task, &actor)
            .await
            .is_ok()
    );
    let other_repo = Uuid::new_v4();
    let mut other_link = source_link
        .try_get::<serde_json::Value>("", "snapshot")
        .unwrap();
    other_link["repository_id"] = json!(other_repo);
    other_link["public_name"] = json!("Unrelated repository");
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO task_repository_links(issue_id,forge_instance_id,repository_id,snapshot,created_by) VALUES($1,$2,$3,$4,$5)",
        vec![queued_task.into(),Uuid::parse_str(other_link["forge_instance_id"].as_str().unwrap()).unwrap().into(),other_repo.into(),other_link.into(),Uuid::parse_str(&actor.subject).unwrap().into()])).await.unwrap();
    assert!(
        repository
            .pending_handoff_materials(queued_task, &actor)
            .await
            .is_err()
    );
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM task_repository_links WHERE issue_id=$1 AND repository_id=$2",
        vec![queued_task.into(), other_repo.into()],
    ))
    .await
    .unwrap();
    assert!(
        repository
            .pending_handoff_materials(queued_task, &actor)
            .await
            .is_ok()
    );
    let frozen=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT payload,generation FROM sdlc_routing_snapshot_inventory WHERE task_id=$1 AND snapshot_id=$2",
        vec![queued_task.into(),requested.routing_snapshot_id.into()])).await.unwrap().unwrap();
    let pending_basis = repository
        .pending_handoff_materials(queued_task, &actor)
        .await
        .unwrap();
    assert_eq!(
        frozen.try_get::<serde_json::Value>("", "payload").unwrap(),
        serde_json::to_value(&pending_basis.routing).unwrap()
    );
    assert!(frozen.try_get::<i64>("", "generation").unwrap() >= 1);
    assert!(
        db.execute_unprepared("UPDATE task_handoff_routing_generations SET generation=generation")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("DELETE FROM task_handoff_routing_generations")
            .await
            .is_err()
    );
    assert!(
        repository
            .request_current_stage_handoff(queued_task, &actor, &queued_command, &observed)
            .await
            .unwrap()
            .1
    );
    let current = repository
        .prepare_current_stage_handoff(queued_task, &actor)
        .await
        .unwrap();
    assert!(
        current
            .blockers
            .contains(&domain::task_handoff::TaskExecutionBlocker::ExecutionBusy)
    );
    assert_eq!(
        (current.state.stage, current.state.status),
        (Stage::Review, Status::Ready)
    );
    assert_eq!(current.state.revision, requested.resulting_revision);
    let worker_read = repository
        .pending_handoff_materials(queued_task, &actor)
        .await
        .unwrap();
    assert_eq!(worker_read.receipt.handoff_id, requested.handoff_id);
    let issues = SeaOrmRepositories::new(Database::connect(url.as_str()).await.unwrap()).issues;
    let actor_id = UserId::from_uuid(Uuid::parse_str(&actor.subject).unwrap());
    assert!(
        issues
            .transition_task_lifecycle(
                IssueId::from_uuid(queued_task),
                actor_id,
                &TransitionCommand {
                    action: Action::Start,
                    expected_revision: current.state.revision,
                    idempotency_key: "cannot-overwrite-queued-request".into(),
                }
            )
            .await
            .is_err()
    );
    let counts=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM task_handoff_materials WHERE execution_requested) AS requests,(SELECT count(*) FROM sdlc_pm_executions) AS pm_runs,(SELECT count(*) FROM sdlc_delivery_assignments) AS assignments".to_owned())).await.unwrap().unwrap();
    assert_eq!(counts.try_get::<i64>("", "requests").unwrap(), 1);
    assert_eq!(counts.try_get::<i64>("", "pm_runs").unwrap(), 0);
    assert_eq!(counts.try_get::<i64>("", "assignments").unwrap(), 0);
    let own_status = repository
        .current_owner_handoff(queued_task, &actor)
        .await
        .unwrap()
        .unwrap();
    assert!(own_status.can_cancel);
    assert_eq!(
        own_status.operation.receipt.handoff_id,
        requested.handoff_id
    );
    assert_eq!(own_status.core_revision, requested.resulting_revision);
    assert!(
        repository
            .current_owner_handoff(queued_task, &scheduler)
            .await
            .is_err()
    );
    let queued = repository
        .requested_handoffs(&scheduler, query.clone())
        .await
        .unwrap();
    assert_eq!(queued.items.len(), 1);
    assert!(queued.next_cursor.is_none());
    assert_eq!(queued.items[0].task_id, queued_task);
    assert_eq!(queued.items[0].handoff_id, requested.handoff_id);
    assert_eq!(queued.items[0].entry.state.stage, Stage::Review);
    let pending = repository
        .requested_handoff_execution(queued_task, &scheduler)
        .await
        .unwrap();
    assert_eq!(pending.consent.handoff_id, requested.handoff_id);
    assert!(pending.assignment.is_none() && pending.lease.is_none() && pending.binding.is_none());
    assert_eq!(pending.task.stage, Stage::Review);
    assert_eq!(pending.task.status, Status::Ready);
    // This isolated metadata fixture uses the real operation ledger/guard.
    // The full archive API/authority path has its own project_lifecycle tests.
    async fn fixture_archive(db: &sea_orm::DatabaseConnection, archived: bool) {
        let operation = Uuid::new_v4();
        db.execute_unprepared(&format!("DO $$ DECLARE revision bigint; operation uuid:='{operation}'; desired boolean:={archived}; BEGIN
            SELECT project_lifecycle_revision INTO revision FROM projects WHERE key='CORE' FOR UPDATE;
            INSERT INTO tracker_project_lifecycle_operations(operation_id,project_id,actor_identity,command,authority_valid_until)
            SELECT operation,id,jsonb_build_object('issuer','https://fixture.example.test','subject','10000000-0000-4000-8000-000000000001'),jsonb_build_object('operation_id',operation,'expected_revision',revision,'archived',desired),floor(extract(epoch FROM clock_timestamp()))::bigint+30 FROM projects WHERE key='CORE';
            PERFORM set_config('pdlc.tracker.project_lifecycle_operation',operation::text,true);
            UPDATE projects SET archived_at=CASE WHEN desired THEN clock_timestamp() ELSE NULL END,project_lifecycle_revision=project_lifecycle_revision+1 WHERE key='CORE';
            END $$;")).await.unwrap();
    }
    // Archival keeps the queued history but removes native launch eligibility.
    fixture_archive(&db, true).await;
    assert!(
        repository
            .requested_handoffs(&scheduler, query.clone())
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        repository
            .requested_handoff_execution(queued_task, &scheduler)
            .await
            .is_err()
    );
    assert!(
        repository
            .prepare_current_stage_handoff(queued_task, &actor)
            .await
            .unwrap()
            .blockers
            .contains(&domain::task_handoff::TaskExecutionBlocker::ProjectReadOnly)
    );
    fixture_archive(&db, false).await;
    assert_eq!(
        repository
            .requested_handoffs(&scheduler, query.clone())
            .await
            .unwrap()
            .items[0]
            .handoff_id,
        requested.handoff_id
    );

    let mut tail = query.clone();
    tail.after = Some(queued_task);
    assert!(
        repository
            .requested_handoffs(&scheduler, tail)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        repository
            .requested_handoff_execution(queued_task, &actor)
            .await
            .is_err()
    );

    let cancel = domain::task_handoff::CancelTaskHandoffCommand {
        expected_revision: current.state.revision,
        idempotency_key: "cancel-original-queued".into(),
    };
    assert!(
        repository
            .cancel_current_stage_handoff(queued_task, requested.handoff_id, &machine, &cancel)
            .await
            .is_err()
    );
    let (cancelled, replayed) = repository
        .cancel_current_stage_handoff(queued_task, requested.handoff_id, &pat_actor, &cancel)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(cancelled.resulting_revision, current.state.revision + 1);
    assert!(
        repository
            .cancel_current_stage_handoff(queued_task, requested.handoff_id, &actor, &cancel)
            .await
            .unwrap()
            .1
    );
    assert!(
        repository
            .pending_handoff_materials(queued_task, &actor)
            .await
            .is_err()
    );
    let available = issues
        .task_lifecycle(IssueId::from_uuid(queued_task), true)
        .await
        .unwrap();
    assert!(!available.execution_busy);
    assert_eq!(
        (available.state.stage, available.state.status),
        (Stage::Review, Status::Ready)
    );
    assert_eq!(available.state.revision, cancelled.resulting_revision);
    assert!(
        repository
            .current_owner_handoff(queued_task, &actor)
            .await
            .unwrap()
            .is_none()
    );

    assert!(
        repository
            .requested_handoffs(&scheduler, query.clone())
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        repository
            .requested_handoff_execution(queued_task, &scheduler)
            .await
            .is_err()
    );

    let resumed = issues
        .transition_task_lifecycle(
            IssueId::from_uuid(queued_task),
            actor_id,
            &TransitionCommand {
                action: Action::Start,
                expected_revision: available.state.revision,
                idempotency_key: "human-after-cancel".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(resumed.state.status, Status::InProgress);
    let again = repository
        .prepare_current_stage_handoff(queued_task, &actor)
        .await
        .unwrap();
    let command_again = command(&again, "second-explicit-request");
    let entry = again.entry.unwrap();
    let mut source_again = again.source.as_ref().unwrap().verification.clone();
    source_again.observed_at = chrono::Utc::now();
    let config_again = app::sdlc_configuration::MatchedConfiguration {
        selection_sha256: app::sdlc_configuration::MatchedConfiguration::selection_hash(
            entry.route(again.routing.as_ref().unwrap()),
            entry.role.key(),
            entry.mode.key(),
        )
        .unwrap(),
        observation_ref: Uuid::new_v4(),
        observed_at: chrono::Utc::now(),
    };
    let observed_again = app::task_handoff_observations::MatchedHandoffInputs::bind(
        &again,
        config_again,
        source_again,
    )
    .unwrap();
    let (second, _) = repository
        .request_current_stage_handoff(queued_task, &actor, &command_again, &observed_again)
        .await
        .unwrap();
    assert_ne!(second.handoff_id, requested.handoff_id);
    let second_routing=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT generation FROM sdlc_routing_snapshot_inventory WHERE task_id=$1 AND snapshot_id=$2",
        vec![queued_task.into(),second.routing_snapshot_id.into()])).await.unwrap().unwrap();
    assert_eq!(
        second_routing.try_get::<i64>("", "generation").unwrap(),
        frozen.try_get::<i64>("", "generation").unwrap() + 1
    );
    let retained_routing=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT payload FROM sdlc_routing_snapshot_inventory WHERE task_id=$1 AND snapshot_id=$2",
        vec![queued_task.into(),requested.routing_snapshot_id.into()])).await.unwrap().unwrap();
    assert_eq!(
        retained_routing
            .try_get::<serde_json::Value>("", "payload")
            .unwrap(),
        frozen.try_get::<serde_json::Value>("", "payload").unwrap()
    );

    assert_eq!(second.entry.state.status, Status::InProgress);
    let pending = repository
        .requested_handoff_execution(queued_task, &scheduler)
        .await
        .unwrap();
    assert_eq!(pending.task.status, Status::InProgress);
    assert_eq!(pending.consent.handoff_id, second.handoff_id);
    assert_eq!(
        repository
            .requested_handoffs(&scheduler, query.clone())
            .await
            .unwrap()
            .items[0]
            .handoff_id,
        second.handoff_id
    );

    // Lost old ACKs must not cancel a successor or silently queue the old operation again.
    assert!(
        repository
            .cancel_current_stage_handoff(queued_task, requested.handoff_id, &actor, &cancel)
            .await
            .unwrap()
            .1
    );
    assert!(
        repository
            .request_current_stage_handoff(queued_task, &actor, &queued_command, &observed)
            .await
            .unwrap()
            .1
    );
    assert!(
        issues
            .task_lifecycle(IssueId::from_uuid(queued_task), true)
            .await
            .unwrap()
            .execution_busy
    );
    assert_eq!(
        repository
            .pending_handoff_materials(queued_task, &actor)
            .await
            .unwrap()
            .receipt
            .handoff_id,
        second.handoff_id
    );
    let wrong_target = domain::task_handoff::CancelTaskHandoffCommand {
        expected_revision: second.resulting_revision,
        idempotency_key: cancel.idempotency_key.clone(),
    };
    assert!(
        repository
            .cancel_current_stage_handoff(queued_task, second.handoff_id, &actor, &wrong_target)
            .await
            .is_err()
    );
    let history = repository
        .handoff_operation(queued_task, &actor, &queued_command.idempotency_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.operation.receipt.handoff_id, requested.handoff_id);
    assert!(history.operation.execution_requested);
    assert_eq!(
        history.operation.cancellation.unwrap().resulting_revision,
        cancelled.resulting_revision
    );
    assert_eq!(
        history.command_hash,
        app::sdlc::canonical_hash(&queued_command).unwrap()
    );
    assert!(
        repository
            .handoff_operation(queued_task, &actor, "unknown-original-key")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .handoff_operation(queued_task, &machine, &queued_command.idempotency_key)
            .await
            .is_err()
    );
    let actual_cancel = domain::task_handoff::CancelTaskHandoffCommand {
        expected_revision: second.resulting_revision,
        idempotency_key: "cancel-second-queued".into(),
    };
    repository
        .cancel_current_stage_handoff(queued_task, second.handoff_id, &actor, &actual_cancel)
        .await
        .unwrap();
    let final_read = issues
        .task_lifecycle(IssueId::from_uuid(queued_task), true)
        .await
        .unwrap();
    assert!(!final_read.execution_busy);
    assert_eq!(
        (final_read.state.stage, final_read.state.status),
        (Stage::Review, Status::InProgress)
    );
    let permits = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM task_handoff_cancel_write_permits".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(permits.try_get::<i64>("", "n").unwrap(), 0);
    let facts = |prepared: &domain::task_handoff::PreparedCurrentStageHandoff| {
        let entry = prepared.entry.unwrap();
        let fleet = app::sdlc_configuration::MatchedConfiguration {
            selection_sha256: app::sdlc_configuration::MatchedConfiguration::selection_hash(
                entry.route(prepared.routing.as_ref().unwrap()),
                entry.role.key(),
                entry.mode.key(),
            )
            .unwrap(),
            observation_ref: Uuid::new_v4(),
            observed_at: chrono::Utc::now(),
        };
        let mut source = prepared.source.as_ref().unwrap().verification.clone();
        source.observed_at = chrono::Utc::now();
        app::task_handoff_observations::MatchedHandoffInputs::bind(prepared, fleet, source).unwrap()
    };
    let before_native = repository
        .prepare_current_stage_handoff(queued_task, &actor)
        .await
        .unwrap();
    let native_request = command(&before_native, "request-genuine-technical");
    let (native_consent, _) = repository
        .request_current_stage_handoff(queued_task, &actor, &native_request, &facts(&before_native))
        .await
        .unwrap();
    let writer = Principal {
        subject: scheduler.subject.clone(),
        trusted_human: false,
        human_session: false,
        scopes: ["task-tracker:write".into()].into(),
    };
    assert!(
        repository
            .prepare_requested_handoff(queued_task, &scheduler)
            .await
            .is_err()
    );
    assert!(
        repository
            .prepare_requested_handoff(queued_task, &actor)
            .await
            .is_err()
    );
    let prepared = repository
        .prepare_requested_handoff(queued_task, &writer)
        .await
        .unwrap();
    let observed = facts(&prepared);
    let producer = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub) VALUES($1,'producer@example.test','producer','Fixture producer','fixture',$2);",vec![producer.into(),producer.to_string().into()])).await.unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![project.into(), producer.into()],
    ))
    .await
    .unwrap();
    let reserve = domain::task_handoff::ReserveTaskHandoffCommand {
        handoff_id: native_consent.handoff_id,
        expected_revision: prepared.state.revision,
        machine_subject: producer.to_string(),
        idempotency_key: "native-first-current-stage".into(),
    };
    // Expire only the stored proof at commit. Real assignment, hold, head and
    // business state must all roll back together; no sleep or fake terminal.
    db.execute_unprepared("CREATE FUNCTION injected_adoption_expiry() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.observations=jsonb_set(NEW.observations,'{valid_until}',to_jsonb((clock_timestamp()-interval '1 second')::text)); RETURN NEW; END $$; CREATE TRIGGER injected_adoption_expiry BEFORE INSERT ON task_handoff_delivery_adoptions FOR EACH ROW EXECUTE FUNCTION injected_adoption_expiry()").await.unwrap();
    let expired = repository
        .reserve_current_stage_handoff(queued_task, &writer, &reserve, &observed)
        .await
        .unwrap_err();
    assert!(
        expired
            .to_string()
            .contains("task_handoff_native_adoption_invalid_or_expired"),
        "{expired:?}"
    );
    db.execute_unprepared("DROP TRIGGER injected_adoption_expiry ON task_handoff_delivery_adoptions;DROP FUNCTION injected_adoption_expiry()").await.unwrap();
    assert_eq!(
        repository
            .prepare_requested_handoff(queued_task, &writer)
            .await
            .unwrap()
            .state,
        prepared.state
    );
    let (reserved, replayed) = repository
        .reserve_current_stage_handoff(queued_task, &writer, &reserve, &observed)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(
        (reserved.task.stage, reserved.task.status),
        (Stage::Review, Status::InProgress)
    );
    assert_eq!(
        reserved.task.runtime,
        domain::sdlc_lifecycle::RuntimeState::Reserved
    );
    let assignment = reserved.assignment.as_ref().unwrap();
    assert_eq!(assignment.handoff_id, native_consent.handoff_id);
    assert_eq!(assignment.machine_subject, producer.to_string());
    assert!(assignment.fencing_token > 0);
    assert!(reserved.lease.is_some());
    assert!(reserved.binding.is_none());
    assert!(reserved.result.is_none());
    assert!(
        repository
            .current_owner_handoff(queued_task, &actor)
            .await
            .unwrap()
            .is_none()
    );
    let again = repository
        .reserve_current_stage_handoff(queued_task, &writer, &reserve, &observed)
        .await
        .unwrap();
    assert!(again.1);
    assert_eq!(
        serde_json::to_value(again.0).unwrap(),
        serde_json::to_value(&reserved).unwrap()
    );
    let busy = issues
        .task_lifecycle(IssueId::from_uuid(queued_task), true)
        .await
        .unwrap();
    assert!(busy.execution_busy);
    assert_eq!(busy.state.revision, prepared.state.revision + 2);
    assert!(
        issues
            .transition_task_lifecycle(
                IssueId::from_uuid(queued_task),
                actor_id,
                &TransitionCommand {
                    action: Action::Complete,
                    expected_revision: busy.state.revision,
                    idempotency_key: "cannot-overwrite-reserved-native".into()
                }
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .cancel_current_stage_handoff(
                queued_task,
                native_consent.handoff_id,
                &actor,
                &domain::task_handoff::CancelTaskHandoffCommand {
                    expected_revision: busy.state.revision,
                    idempotency_key: "cannot-cancel-claimed-native".into()
                }
            )
            .await
            .is_err()
    );
    let storage=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM sdlc_delivery_assignments) AS assignments,(SELECT count(*) FROM sdlc_delivery_holds) AS holds,(SELECT count(*) FROM sdlc_pm_executions) AS pm,(SELECT count(*) FROM task_handoff_delivery_seed_permits) AS permits".to_owned())).await.unwrap().unwrap();
    assert_eq!(storage.try_get::<i64>("", "assignments").unwrap(), 1);
    assert_eq!(storage.try_get::<i64>("", "holds").unwrap(), 1);
    assert_eq!(storage.try_get::<i64>("", "pm").unwrap(), 0);
    assert_eq!(storage.try_get::<i64>("", "permits").unwrap(), 0);
    // Classification of retained human origin is not a write capability.
    let origin=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT task_handoff_current_delivery_origin(task_id,state-'lifecycle'-'runtime_state'-'waiting_reason') AS retained FROM sdlc_tasks WHERE task_id=$1",vec![queued_task.into()])).await.unwrap().unwrap();
    assert!(origin.try_get::<bool>("", "retained").unwrap());
    assert!(db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"UPDATE sdlc_tasks SET state=state-'lifecycle'-'runtime_state'-'waiting_reason',manual_control=true WHERE task_id=$1",vec![queued_task.into()])).await.is_err());
    let ready_task = Uuid::parse_str("50000000-0000-4000-8000-000000000001").unwrap();
    let ready_state = issues
        .task_lifecycle(IssueId::from_uuid(ready_task), true)
        .await
        .unwrap();
    assert_eq!(ready_state.state.stage, Stage::Backlog);
    let ready_state = issues
        .transition_task_lifecycle(
            IssueId::from_uuid(ready_task),
            actor_id,
            &TransitionCommand {
                action: Action::Start,
                expected_revision: ready_state.state.revision,
                idempotency_key: "human-start-backlog".into(),
            },
        )
        .await
        .unwrap();

    issues
        .transition_task_lifecycle(
            IssueId::from_uuid(ready_task),
            actor_id,
            &TransitionCommand {
                action: Action::Complete,
                expected_revision: ready_state.state.revision,
                idempotency_key: "human-ready-analysis".into(),
            },
        )
        .await
        .unwrap();
    for (next_task, slot_available) in [(ready_task, true), (task, false)] {
        let plan = repository
            .prepare_current_stage_handoff(next_task, &actor)
            .await
            .unwrap();
        let key = if slot_available {
            "ready-analysis-request"
        } else {
            "third-task-waits"
        };
        let (consent, _) = repository
            .request_current_stage_handoff(next_task, &actor, &command(&plan, key), &facts(&plan))
            .await
            .unwrap();
        let admission = repository
            .prepare_requested_handoff(next_task, &writer)
            .await
            .unwrap();
        let request = domain::task_handoff::ReserveTaskHandoffCommand {
            handoff_id: consent.handoff_id,
            expected_revision: admission.state.revision,
            machine_subject: producer.to_string(),
            idempotency_key: key.into(),
        };
        let result = repository
            .reserve_current_stage_handoff(next_task, &writer, &request, &facts(&admission))
            .await;
        if slot_available {
            let current = result.unwrap().0;
            assert_eq!(
                (current.task.stage, current.task.status),
                (Stage::Analysis, Status::Ready)
            );
            assert!(current.assignment.unwrap().fencing_token > assignment.fencing_token);
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("technical pool capacity held")
            );
            let queued = repository
                .current_owner_handoff(next_task, &actor)
                .await
                .unwrap()
                .unwrap();
            assert!(queued.can_cancel);
            repository
                .cancel_current_stage_handoff(
                    next_task,
                    consent.handoff_id,
                    &actor,
                    &domain::task_handoff::CancelTaskHandoffCommand {
                        expected_revision: queued.core_revision,
                        idempotency_key: "cancel-third-waiting".into(),
                    },
                )
                .await
                .unwrap();
        }
    }
    let pool=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM sdlc_delivery_holds) AS held,(SELECT count(*) FROM task_handoff_delivery_adoptions) AS admissions,(SELECT count(*) FROM sdlc_pm_executions) AS pm".to_owned())).await.unwrap().unwrap();
    assert_eq!(pool.try_get::<i64>("", "held").unwrap(), 2);
    assert_eq!(pool.try_get::<i64>("", "admissions").unwrap(), 2);
    assert_eq!(pool.try_get::<i64>("", "pm").unwrap(), 0);
    // Real owner commands must not acknowledge drain while the actual technical
    // assignments above still hold capacity. A foreign namespace is independent.
    use shared::{
        space_access::{AdmissionValidity, CentralIdentity, SpaceRef},
        space_lifecycle::{
            DesiredSpaceState, SpaceMetadata, SpaceOwnerAuthority, SpaceOwnerCommandV2,
        },
    };
    let instance = Uuid::parse_str("40000000-0000-4000-8000-000000000001").unwrap();
    // This ignored integration test is the only selected test in its owned process.
    unsafe {
        std::env::set_var(
            "TT_NAMESPACE__REGISTRY_INSTANCE_ID",
            namespace.registry_instance_id.to_string(),
        );
        std::env::set_var("TT_NAMESPACE__INSTANCE_ID", instance.to_string());
    }
    let now = chrono::Utc::now().timestamp();
    let authority = SpaceOwnerAuthority::confirmed(
        CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: Uuid::new_v4().to_string(),
        },
        AdmissionValidity::confirmed(now, now + 60, i64::MAX).unwrap(),
    )
    .unwrap();
    let mut source_command = SpaceOwnerCommandV2 {
        schema_version: 2,
        space: SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: namespace.namespace_id,
        },
        participant_instance_id: instance,
        operation_id: Uuid::new_v4(),
        revision: 1,
        desired_state: DesiredSpaceState::Active,
        metadata: SpaceMetadata {
            slug: "source-space".into(),
            name: "Source space".into(),
            description: "Owned fixture".into(),
            responsible_subject: actor.subject.clone(),
        },
    };
    let ack = infra::space_owner::apply(&db, &source_command, &authority)
        .await
        .unwrap();
    assert!(ack.matches(&source_command));
    let mut neighbor = source_command.clone();
    neighbor.space.space_id = Uuid::new_v4();
    neighbor.operation_id = Uuid::new_v4();
    neighbor.metadata.slug = "neighbor-space".into();
    assert!(
        infra::space_owner::apply(&db, &neighbor, &authority)
            .await
            .unwrap()
            .matches(&neighbor)
    );
    neighbor.operation_id = Uuid::new_v4();
    neighbor.revision = 2;
    neighbor.desired_state = DesiredSpaceState::Archived;
    assert!(
        infra::space_owner::apply(&db, &neighbor, &authority)
            .await
            .unwrap()
            .matches(&neighbor)
    );
    source_command.operation_id = Uuid::new_v4();
    source_command.revision = 2;
    source_command.desired_state = DesiredSpaceState::Archived;
    let archive = infra::space_owner::apply(&db, &source_command, &authority)
        .await
        .unwrap();
    assert!(
        !archive.drained,
        "technical assignments must prevent a false archive acknowledgement"
    );
    assert!(!archive.matches(&source_command));
    assert!(
        !infra::space_owner::apply(&db, &source_command, &authority)
            .await
            .unwrap()
            .drained
    );
    let state = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT state FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2",
            vec![
                namespace.registry_instance_id.into(),
                namespace.namespace_id.into(),
            ],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.try_get::<String>("", "state").unwrap(), "archiving");
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
