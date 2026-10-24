//! Real PostgreSQL qualification of the scalar reference barrier. This is not
//! acceptance of the complete owner: typed JSON references and RPCs are separate.
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, IsolationLevel,
    Statement, TransactionTrait,
};
use std::time::Duration;

async fn connection(url: &str, schema: Option<&str>) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url);
    options.max_connections(1).min_connections(1);
    let db = Database::connect(options).await.unwrap();
    let database: String = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert_eq!(
        database, "tasktracker_migration_test",
        "refuse application database"
    );
    if let Some(schema) = schema {
        db.execute_unprepared(&format!("SET search_path TO {schema}"))
            .await
            .unwrap();
    }
    db
}

async fn freeze(db: &DatabaseConnection, subject: &str) {
    let tx = db.begin().await.unwrap();
    tx.execute_unprepared(
        "SELECT pg_advisory_xact_lock(714402216002::bigint);
        UPDATE tracker_identity_fence_epoch SET revision=revision+1 WHERE singleton",
    )
    .await
    .unwrap();
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO tracker_identity_deletion_operations(operation_id,issuer,subject,inventory_revision,
            request,state,fence,revision,reference_count)
         VALUES(gen_random_uuid(),'https://auth.example.test',$1,gen_random_uuid(),'{}','frozen',gen_random_uuid(),1,0)",
        [subject.into()])).await.unwrap();
    tx.commit().await.unwrap();
}

async fn insert_user(
    db: &impl ConnectionTrait,
    id: &str,
    subject: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
         VALUES($1::text::uuid,$1::text||'@example.test',$1::text,'Synthetic','!',$2)",
        [id.into(), subject.map(str::to_owned).into()],
    ))
    .await
    .map(|_| ())
}

#[tokio::test]
#[ignore = "requires dedicated tasktracker_migration_test PostgreSQL database"]
async fn scalar_writers_drain_and_cannot_bypass_frozen_subject_or_old_snapshot() {
    let url =
        std::env::var("TT_MIGRATION_TEST_DATABASE_URL").expect("dedicated migration database");
    let db = connection(&url, None).await;
    let schema: String = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT 'identity_fences_'||replace(gen_random_uuid()::text,'-','') AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    db.execute_unprepared(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .await
    .unwrap();
    let cleanup = connection(&url, None).await;
    let work_schema = schema.clone();
    let result = tokio::spawn(async move {
    let schema = work_schema;
    Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared("INSERT INTO tracker_identity_legacy_issuer(issuer) VALUES('https://auth.example.test')")
        .await.unwrap();
    let writer = connection(&url, Some(&schema)).await;
    let owner = connection(&url, Some(&schema)).await;
    let old_snapshot = connection(&url, Some(&schema)).await;

    // The owner cannot pass an already admitted writer, even a raw legacy SQL
    // writer which has no application-level awareness of identity deletion.
    let tx = writer.begin().await.unwrap();
    insert_user(&tx,"00000000-0000-4000-8000-000000000001",Some("existing-central")).await.unwrap();
    let owner_pid: i32 = owner.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT pg_backend_pid() AS pid")).await.unwrap().unwrap().try_get("", "pid").unwrap();
    let pending = tokio::spawn(async move { freeze(&owner,"frozen-central").await; owner });
    struct AbortOnDrop(tokio::task::AbortHandle);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) { self.0.abort(); }
    }
    let _pending_guard = AbortOnDrop(pending.abort_handle());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = db.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
                "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND NOT granted) AS waiting",[owner_pid.into()]))
                .await.unwrap().unwrap().try_get("", "waiting").unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("owner must wait for the live writer");
    assert!(!pending.is_finished());
    tx.commit().await.unwrap();
    let owner = tokio::time::timeout(Duration::from_secs(10), pending).await.unwrap().unwrap();

    let error=insert_user(&db,"00000000-0000-4000-8000-000000000002",Some("frozen-central")).await.unwrap_err();
    assert!(error.to_string().contains("identity_deletion_fenced"));
    let error=db.execute_unprepared("INSERT INTO tracker_identity_aliases(local_user_id,central_subject)
        VALUES('00000000-0000-4000-8000-000000000003','frozen-central')").await.unwrap_err();
    assert!(error.to_string().contains("identity_deletion_fenced"),"operator backfill cannot bypass the same barrier");
    insert_user(&db,"00000000-0000-4000-8000-000000000004",None).await.unwrap();
    let error=db.execute_unprepared("UPDATE users SET central_sub='frozen-central'
        WHERE id='00000000-0000-4000-8000-000000000004'").await.unwrap_err();
    assert!(error.to_string().contains("identity_deletion_fenced"),"linking an old local profile is a new reference");
    insert_user(&db,"00000000-0000-4000-8000-000000000005",Some("unrelated-central")).await.unwrap();
    let native_refs:i64=db.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT count(*) AS n FROM tracker_identity_local_ref('00000000-0000-4000-8000-000000000004')"))
        .await.unwrap().unwrap().try_get("","n").unwrap();
    assert_eq!(native_refs,0,"local UUID is not a guessed central subject");
    insert_user(&db,"00000000-0000-4000-8000-000000000008",None).await.unwrap();
    db.execute_unprepared("DELETE FROM users WHERE id='00000000-0000-4000-8000-000000000008'")
        .await.unwrap();
    let error=insert_user(&db,"00000000-0000-4000-8000-000000000008",Some("other-central")).await.unwrap_err();
    assert!(error.to_string().contains("identity_alias_rebind_denied"),"recycled native UUID cannot reattribute old history");

    // Cancellation releases the barrier; the existing native profile can then
    // be explicitly linked. The alias transitions in the same transaction.
    db.execute_unprepared("BEGIN; SELECT pg_advisory_xact_lock(714402216002::bigint);
        UPDATE tracker_identity_fence_epoch SET revision=revision+1 WHERE singleton;
        UPDATE tracker_identity_deletion_operations SET state='released',fence=NULL WHERE subject='frozen-central'; COMMIT;
        UPDATE users SET central_sub='frozen-central' WHERE id='00000000-0000-4000-8000-000000000004'")
        .await.unwrap();
    let linked:String=db.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT subject FROM tracker_identity_local_ref('00000000-0000-4000-8000-000000000004')"))
        .await.unwrap().unwrap().try_get("","subject").unwrap();
    assert_eq!(linked,"frozen-central");

    // A snapshot created before freeze must serialize, not observe the old
    // absence of a fence after obtaining the advisory lock later.
    let old = old_snapshot.begin_with_config(Some(IsolationLevel::RepeatableRead),None).await.unwrap();
    old.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT count(*) FROM tracker_identity_deletion_operations")).await.unwrap();
    freeze(&owner,"later-frozen-central").await;
    let error=insert_user(&old,"00000000-0000-4000-8000-000000000006",Some("later-frozen-central")).await.unwrap_err();
    assert!(error.to_string().contains("could not serialize"),"epoch row must invalidate the stale snapshot");
    old.rollback().await.unwrap();

    db.execute_unprepared("DELETE FROM tracker_identity_fence_epoch").await.unwrap();
    let error=insert_user(&db,"00000000-0000-4000-8000-000000000007",None).await.unwrap_err();
    assert!(error.to_string().contains("identity_fence_epoch_missing"));
    writer.close().await.unwrap();owner.close().await.unwrap();old_snapshot.close().await.unwrap();
    db.close().await.unwrap();
    }).await;
    cleanup
        .execute_unprepared(&format!(
            "SET search_path TO public; DROP SCHEMA {schema} CASCADE"
        ))
        .await
        .unwrap();
    cleanup.close().await.unwrap();
    assert!(
        result.is_ok(),
        "isolated scalar reference barrier checks failed"
    );
}
