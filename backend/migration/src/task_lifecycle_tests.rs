use super::*;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

async fn scalar(db: &impl ConnectionTrait, query: &str) -> serde_json::Value {
    db.query_one(Statement::from_string(
        DbBackend::Postgres,
        query.to_owned(),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "v")
    .unwrap()
}
#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn populated_import_is_replayable_and_atomic_creation_and_guards_preserve_history() {
    let url =
        std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").expect("owned test database required");
    let db = Database::connect(url).await.unwrap();
    let name = scalar(&db, "SELECT to_jsonb(current_database()) AS v").await;
    assert!(name.as_str().unwrap().ends_with("_task_lifecycle_test"));
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(to_regclass('public.issues') IS NULL) AS v"
        )
        .await,
        serde_json::json!(true)
    );
    Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(include_str!("task_lifecycle_fixture.sql"))
        .await
        .unwrap();
    let before=scalar(&db,"SELECT jsonb_agg(to_jsonb(i)-ARRAY['status_id','updated_at'] ORDER BY id) AS v FROM issues i").await;
    assert!(
        db.execute_unprepared(
            "SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001','[]'::jsonb)"
        )
        .await
        .is_err()
    );
    assert_eq!(
        scalar(&db, "SELECT to_jsonb(count(*)) AS v FROM sdlc_tasks").await,
        serde_json::json!(0)
    );
    db.execute_unprepared("INSERT INTO statuses(id,name,category,position,is_default,is_closed) VALUES('90000000-0000-4000-8000-000000000001','Custom foreign status','todo',0,false,false); UPDATE issues SET status_id='90000000-0000-4000-8000-000000000001' WHERE key='CORE-1'").await.unwrap();
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(blocker) AS v FROM task_lifecycle_inventory WHERE task_key='CORE-1'"
        )
        .await,
        serde_json::json!("unknown_legacy_status")
    );
    assert!(db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.is_err());
    assert_eq!(
        scalar(&db, "SELECT to_jsonb(count(*)) AS v FROM sdlc_tasks").await,
        serde_json::json!(0)
    );
    db.execute_unprepared(
        "UPDATE issues SET status_id='00000000-0000-0000-0000-000000000001' WHERE key='CORE-1'",
    )
    .await
    .unwrap();
    db.execute_unprepared(r#"CREATE FUNCTION fixture_refuse_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        IF NEW.task_id='50000000-0000-4000-8000-000000000002'::uuid THEN RAISE EXCEPTION 'fixture import failure'; END IF;
        RETURN NEW; END $$;
        CREATE TRIGGER fixture_refuse_import BEFORE INSERT ON task_lifecycle_history FOR EACH ROW EXECUTE FUNCTION fixture_refuse_import();"#).await.unwrap();
    assert!(db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.is_err());
    assert_eq!(
        scalar(&db, "SELECT to_jsonb(count(*)) AS v FROM sdlc_tasks").await,
        serde_json::json!(0)
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(count(*)) AS v FROM task_lifecycle_events"
        )
        .await,
        serde_json::json!(0)
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(enabled) AS v FROM task_lifecycle_control WHERE singleton"
        )
        .await,
        serde_json::json!(false)
    );
    db.execute_unprepared("DROP TRIGGER fixture_refuse_import ON task_lifecycle_history; DROP FUNCTION fixture_refuse_import()").await.unwrap();
    db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.unwrap();
    let states=scalar(&db,"SELECT jsonb_agg(jsonb_build_array(t.state->>'stage',t.state->>'status') ORDER BY i.key) AS v FROM issues i JOIN sdlc_tasks t ON t.task_id=i.id").await;
    assert_eq!(
        states,
        serde_json::json!([
            ["Backlog", "Ready"],
            ["Development", "InProgress"],
            ["Deployment", "Completed"],
            ["Review", "Ready"]
        ])
    );
    assert_eq!(before,scalar(&db,"SELECT jsonb_agg(to_jsonb(i)-ARRAY['status_id','updated_at'] ORDER BY id) AS v FROM issues i").await);
    db.execute_unprepared(
        "SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001','[]'::jsonb)",
    )
    .await
    .unwrap();
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(count(*)) AS v FROM task_lifecycle_history"
        )
        .await,
        serde_json::json!(4)
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(count(*)) AS v FROM sdlc_pm_executions"
        )
        .await,
        serde_json::json!(0)
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(count(*)) AS v FROM sdlc_delivery_results"
        )
        .await,
        serde_json::json!(0)
    );
    assert!(
        db.execute_unprepared(
            "UPDATE issues SET status_id='00000000-0000-0000-0000-000000000002' WHERE key='CORE-1'"
        )
        .await
        .is_err()
    );
    assert!(db.execute_unprepared("UPDATE sdlc_tasks SET state=jsonb_set(state,'{stage}','\"Deployment\"') WHERE task_id='50000000-0000-4000-8000-000000000001'").await.is_err());
    db.execute_unprepared("UPDATE task_lifecycle_control SET enabled=true WHERE singleton")
        .await
        .unwrap();
    db.execute_unprepared(r#"
        INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,reporter_id,priority,labels,position,time_spent_seconds)
        VALUES('50000000-0000-4000-8000-000000000005','20000000-0000-4000-8000-000000000001','CORE-5','Task',
            '00000000-0000-0000-0000-000000000001','Fresh task','10000000-0000-4000-8000-000000000001','Medium','[]',5,0);
    "#).await.unwrap();
    assert_eq!(scalar(&db,"SELECT jsonb_build_array(state->>'stage',state->>'status',core_revision) AS v FROM sdlc_tasks WHERE task_id='50000000-0000-4000-8000-000000000005'").await,serde_json::json!(["Draft","Ready",1]));
    assert_eq!(
        scalar(
            &db,
            "SELECT to_jsonb(count(*)) AS v FROM task_lifecycle_history"
        )
        .await,
        serde_json::json!(5)
    );
    db.execute_unprepared(r#"BEGIN; SET CONSTRAINTS ALL DEFERRED;
        INSERT INTO projects(id,key,name,owner_id,default_board_id)
        VALUES('20000000-0000-4000-8000-000000000002','FRESH','Fresh project','10000000-0000-4000-8000-000000000001','30000000-0000-4000-8000-000000000002');
        INSERT INTO boards(id,project_id,name,columns) VALUES('30000000-0000-4000-8000-000000000002','20000000-0000-4000-8000-000000000002','Board',
            '[{"id":"00000000-0000-0000-0000-000000000001","wip_limit":null},{"id":"00000000-0000-0000-0000-000000000002","wip_limit":5},{"id":"00000000-0000-0000-0000-000000000004","wip_limit":null},{"id":"00000000-0000-0000-0000-000000000003","wip_limit":null}]');
        COMMIT;"#).await.unwrap();
    assert_eq!(scalar(&db,"SELECT to_jsonb(jsonb_array_length(columns::jsonb)) AS v FROM boards WHERE id='30000000-0000-4000-8000-000000000002'").await,serde_json::json!(9));
    assert_eq!(scalar(&db,"SELECT to_jsonb(wip_limit) AS v FROM task_lifecycle_board_limits WHERE project_id='20000000-0000-4000-8000-000000000002' AND stage='Development'").await,serde_json::json!(5));
    assert!(
        db.execute_unprepared(
            "UPDATE boards SET columns='[]' WHERE id='30000000-0000-4000-8000-000000000002'"
        )
        .await
        .is_err()
    );
    assert!(
        db.execute_unprepared("UPDATE task_lifecycle_control SET enabled=false WHERE singleton")
            .await
            .is_err()
    );
}
