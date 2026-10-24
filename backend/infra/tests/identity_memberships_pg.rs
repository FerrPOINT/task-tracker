//! Actual repository + fully migrated PostgreSQL. No Auth or HTTP stub stands in for storage.
use infra::repos::SeaOrmRepositories;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
use shared::AppError;
use uuid::Uuid;

fn sql(text: &str, values: Vec<sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, text, values)
}

#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL"]
async fn exact_subject_metadata_is_read_only_paged_and_snapshot_consistent() {
    let raw =
        std::env::var("TT_IDENTITY_TEST_DATABASE_URL").expect("dedicated PostgreSQL required");
    let admin = Database::connect(&raw).await.unwrap();
    let name: String = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert_eq!(
        name, "tasktracker_identity_test",
        "refuse application database"
    );
    let schema = format!("identity_projection_{}", Uuid::new_v4().simple());
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&raw).unwrap();
    assert!(!url.query_pairs().any(|(key, _)| key == "options"));
    url.query_pairs_mut()
        .append_pair("options", &format!("-c search_path={schema}"));
    let db = Database::connect(url.to_string()).await.unwrap();
    // SeaORM's enabled mock feature deliberately removes DatabaseConnection::Clone.
    // Distinct scoped pools preserve the production feature graph.
    let cleanup = Database::connect(url.to_string()).await.unwrap();
    let repos_db = Database::connect(url.to_string()).await.unwrap();
    let result = tokio::spawn(async move {
        Migrator::up(&db, None).await.unwrap();
        let subject=Uuid::new_v4();let local=Uuid::new_v4();let other=Uuid::new_v4();let shadow=Uuid::new_v4();
        for (id,binding,active) in [(local,Some(subject.to_string()),false),(other,Some(Uuid::new_v4().to_string()),true),(shadow,None,true)] {
            db.execute(sql("INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,'Synthetic','!',$4,$5)",vec![id.into(),format!("{id}@example.test").into(),id.to_string().into(),binding.into(),active.into()])).await.unwrap();
        }
        let owned=Uuid::new_v4();let member=Uuid::new_v4();let excluded=Uuid::new_v4();
        for (id,key,owner) in [(owned,"ZZ",local),(member,"AA",other),(excluded,"CC",other)] {
            db.execute(sql("INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,$2,'Equal display name',$3,$4)",vec![id.into(),key.into(),owner.into(),Uuid::new_v4().into()])).await.unwrap();
        }
        for (project,user) in [(owned,local),(member,local),(excluded,shadow)] {
            db.execute(sql("INSERT INTO project_members(project_id,user_id,role,joined_at) VALUES($1,$2,'viewer','2026-01-02T03:04:05Z')",vec![project.into(),user.into()])).await.unwrap();
        }
        let registry=Uuid::new_v4();let first=Uuid::new_v4();let second=Uuid::new_v4();
        for space in [first,second] {
            db.execute(sql("INSERT INTO tracker_space_catalog(registry_instance_id,space_id,state,admin_revision) VALUES($1,$2,'active',1)",vec![registry.into(),space.into()])).await.unwrap();
            db.execute(sql("INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",vec![member.into(),registry.into(),space.into()])).await.unwrap();
        }
        let repos=SeaOrmRepositories::new(repos_db);
        let before: i64=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM users"))
            .await.unwrap().unwrap().try_get("","n").unwrap();
        let absent=repos.members.identity_memberships(Uuid::new_v4(),25,0).await.unwrap();
        assert!(absent.items.is_empty());assert_eq!(absent.total,0);
        let page=repos.members.identity_memberships(subject,1,0).await.unwrap();
        assert_eq!(page.total,2);assert_eq!(page.items.len(),1);assert_eq!(page.items[0].project_id,member);
        assert_eq!(page.items[0].role,"viewer");assert_eq!(page.items[0].joined_at.unwrap().timestamp(),1767323045);
        assert_eq!(page.items[0].spaces.len(),2);
        assert!(page.items[0].spaces.iter().all(|space|space.registry_instance_id==registry));
        assert!(page.items[0].spaces.iter().any(|space|space.space_id==first));
        assert!(page.items[0].spaces.iter().any(|space|space.space_id==second));
        let next=repos.members.identity_memberships(subject,1,1).await.unwrap();
        assert_eq!(next.total,2);assert_eq!(next.items.len(),1);assert_eq!(next.items[0].project_id,owned);
        assert_eq!(next.items[0].role,"owner");assert!(next.items[0].joined_at.is_none());
        let empty=repos.members.identity_memberships(subject,1,2).await.unwrap();assert_eq!(empty.total,2);assert!(empty.items.is_empty());
        assert!(repos.members.identity_memberships(Uuid::nil(),25,0).await.is_err());
        let after:i64=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM users"))
            .await.unwrap().unwrap().try_get("","n").unwrap();
        assert_eq!(before,after,"metadata reads must not provision a local identity");

        // First profile SELECT establishes the repeatable-read snapshot. Hold the
        // projects relation so the count waits, then commit another owned project.
        let held=db.begin().await.unwrap();
        held.execute_unprepared("LOCK TABLE projects IN ACCESS EXCLUSIVE MODE").await.unwrap();
        let pid:i32=held.query_one(Statement::from_string(DbBackend::Postgres,"SELECT pg_backend_pid() AS pid"))
            .await.unwrap().unwrap().try_get("","pid").unwrap();
        let reader=repos.members.clone();
        let pending=tokio::spawn(async move{reader.identity_memberships(subject,100,0).await.unwrap()});
        let mut blocked=false;
        for _ in 0..100 {
            blocked=db.query_one(sql("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid))) AS blocked",vec![pid.into()]))
                .await.unwrap().unwrap().try_get("","blocked").unwrap();
            if blocked {break;}
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(blocked,"projection did not reach the deterministic relation barrier");
        held.execute(sql("INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'MM','Concurrent project',$2,$3)",vec![Uuid::new_v4().into(),local.into(),Uuid::new_v4().into()])).await.unwrap();
        held.commit().await.unwrap();
        let stable=pending.await.unwrap();assert_eq!(stable.total,2);assert_eq!(stable.items.len(),2);
        let fresh=repos.members.identity_memberships(subject,100,0).await.unwrap();assert_eq!(fresh.total,3);assert_eq!(fresh.items.len(),3);

        // Fault injection is confined to this disposable schema: corrupted legacy
        // multiplicity must become unavailable, never an arbitrary first profile.
        db.execute_unprepared("DROP INDEX users_central_sub_idx").await.unwrap();
        let duplicate=Uuid::new_v4();
        db.execute(sql("INSERT INTO users(id,email,username,display_name,password_hash,central_sub) VALUES($1,$2,$3,'Ambiguous synthetic','!',$4)",vec![duplicate.into(),format!("{duplicate}@example.test").into(),duplicate.to_string().into(),subject.to_string().into()])).await.unwrap();
        assert!(matches!(repos.members.identity_memberships(subject,25,0).await,Err(AppError::Unavailable(_))));
        drop(repos);
    }).await;
    cleanup.close().await.unwrap();
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    admin.close().await.unwrap();
    assert!(
        result.is_ok(),
        "isolated repository projection checks failed"
    );
}
