//! Actual owner protocol transactions on a dedicated, fully migrated PostgreSQL database.
use infra::identity_deletion::IdentityDeletionStore;
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
    TransactionTrait,
};
use shared::identity_deletion::*;
use shared::space_access::CentralIdentity;
use std::{
    future::Future,
    time::{Duration, Instant},
};
use uuid::Uuid;
const ISSUER: &str = "https://auth.example.test";
fn sql(query: &str, args: impl IntoIterator<Item = sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, query, args)
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("connect dedicated identity fixture");
    let database: String = db
        .query_one(sql("SELECT current_database() AS name", []))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert_eq!(
        database, "tasktracker_identity_test",
        "refuse application database"
    );
    db
}

async fn isolated<F, Fut>(work: F)
where
    F: FnOnce(DatabaseConnection, String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let url = std::env::var("TT_IDENTITY_TEST_DATABASE_URL")
        .expect("dedicated tasktracker_identity_test URL required");
    let cleanup = connect(&url).await;
    let schema = format!("identity_contract_{}", Uuid::new_v4().simple());
    cleanup
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut schema_url = reqwest::Url::parse(&url).expect("test connection URL");
    schema_url
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let schema_url = schema_url.to_string();
    let work_schema = schema.clone();
    // The task owns every work connection; a panic drops it before exact cleanup.
    let result = tokio::spawn(async move {
        let db = connect(&schema_url).await;
        let selected: String = db
            .query_one(sql("SELECT current_schema() AS name", []))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "name")
            .unwrap();
        assert_eq!(
            selected, work_schema,
            "all connections must use the isolated schema"
        );
        Migrator::up(&db, None).await.unwrap();
        db.execute(sql(
            "INSERT INTO tracker_identity_legacy_issuer(issuer) VALUES($1)",
            [ISSUER.into()],
        ))
        .await
        .unwrap();
        work(db, schema_url).await;
    })
    .await;
    cleanup
        .execute_unprepared("SET statement_timeout='15s'")
        .await
        .unwrap();
    let removed = cleanup
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
    cleanup.close().await.unwrap();
    removed.expect("clean only this test's exact schema after success or panic");
    if let Err(error) = result {
        if error.is_panic() {
            std::panic::resume_unwind(error.into_panic());
        }
        panic!("identity contract task was cancelled");
    }
}
fn request() -> DeletionRequest {
    DeletionRequest {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: CentralIdentity {
            issuer: ISSUER.into(),
            subject: Uuid::new_v4().to_string(),
        },
        inventory_revision: Uuid::new_v4(),
    }
}
fn window() -> IdentityDeletionWindow {
    IdentityDeletionWindow::restricted(
        Instant::now() + Duration::from_secs(30),
        chrono::Utc::now().timestamp() + 30,
    )
}
async fn insert(db: &impl ConnectionTrait, subject: &str) -> Result<(), sea_orm::DbErr> {
    db.execute(sql("INSERT INTO users(id,email,username,display_name,password_hash,central_sub) VALUES($1,$2,$3,'Synthetic','!',$4)", [Uuid::new_v4().into(),format!("{}@example.test",Uuid::new_v4()).into(),Uuid::new_v4().to_string().into(),subject.into()])).await.map(|_|())
}
#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn complete_owner_replay_count_and_release_before_late_freeze() {
    isolated(|db, _| async move {
        let owner = IdentityDeletionStore::new(&db);
        let r = request();
        let w = window();
        assert_eq!(
            owner
                .identity_owner_check(&r, &w)
                .await
                .unwrap()
                .reference_count,
            0
        );
        let receipt = owner.identity_owner_freeze(&r, &w).await.unwrap();
        assert_eq!(receipt.owner, "task-tracker");
        assert_eq!(receipt, owner.identity_owner_freeze(&r, &w).await.unwrap());
        assert!(
            insert(&db, &r.identity.subject)
                .await
                .unwrap_err()
                .to_string()
                .contains("identity_deletion_fenced")
        );
        let mut conflict = r.clone();
        conflict.inventory_revision = Uuid::new_v4();
        assert!(matches!(
            owner.identity_owner_freeze(&conflict, &w).await,
            Err(shared::AppError::Conflict(_))
        ));
        let mut competing = r.clone();
        competing.operation_id = Uuid::new_v4();
        assert!(owner.identity_owner_freeze(&competing, &w).await.is_err());
        owner.identity_owner_release(&r, &w).await.unwrap();
        assert!(owner.identity_owner_freeze(&r, &w).await.is_err());
        insert(&db, &r.identity.subject).await.unwrap();
        let summary = owner.identity_owner_check(&competing, &w).await.unwrap();
        assert_eq!(
            summary.reference_count, 2,
            "profile and retained central binding each count"
        );
        db.execute(sql(
            "DELETE FROM users WHERE central_sub=$1",
            [r.identity.subject.clone().into()],
        ))
        .await
        .unwrap();
        assert_eq!(
            owner
                .identity_owner_check(&competing, &w)
                .await
                .unwrap()
                .reference_count,
            1,
            "deleted local profile cannot hide its central history"
        );
        let late = request();
        owner.identity_owner_release(&late, &w).await.unwrap();
        assert!(owner.identity_owner_freeze(&late, &w).await.is_err());
        db.close().await.unwrap();
    })
    .await;
}
#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn owner_missing_epoch_and_wrong_issuer_are_closed() {
    isolated(|db, _| async move {
        let owner = IdentityDeletionStore::new(&db);
        let mut r = request();
        r.identity.issuer = "https://other.example.test".into();
        assert!(matches!(
            owner.identity_owner_check(&r, &window()).await,
            Err(shared::AppError::Unavailable(_))
        ));
        r.identity.issuer = ISSUER.into();
        db.execute_unprepared("DELETE FROM tracker_identity_fence_epoch")
            .await
            .unwrap();
        assert!(matches!(
            owner.identity_owner_freeze(&r, &window()).await,
            Err(shared::AppError::Unavailable(_))
        ));
        let n: i64 = db
            .query_one(sql(
                "SELECT count(*) AS n FROM tracker_identity_deletion_operations",
                [],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap();
        assert_eq!(n, 0);
        db.close().await.unwrap();
    })
    .await;
}
#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn owner_does_not_extend_original_authority_after_writer_wait() {
    isolated(|db, url| async move {
        let blocker = connect(&url).await;
        let tx = blocker.begin().await.unwrap();
        tx.execute_unprepared("SELECT pg_advisory_xact_lock_shared(714402216002::bigint)")
            .await
            .unwrap();
        let pid: i32 = db
            .query_one(sql("SELECT pg_backend_pid() AS pid", []))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "pid")
            .unwrap();
        let r = request();
        let w = IdentityDeletionWindow::restricted(
            Instant::now() + Duration::from_millis(100),
            chrono::Utc::now().timestamp() + 30,
        );
        let pending = tokio::spawn(async move {
            let result = IdentityDeletionStore::new(&db)
                .identity_owner_freeze(&r, &w)
                .await;
            (result, db)
        });
        struct Abort(tokio::task::AbortHandle);
        impl Drop for Abort {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _guard = Abort(pending.abort_handle());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let waiting: bool = tx
                    .query_one(sql(
                        "SELECT cardinality(pg_blocking_pids($1))>0 AS waiting",
                        [pid.into()],
                    ))
                    .await
                    .unwrap()
                    .unwrap()
                    .try_get("", "waiting")
                    .unwrap();
                if waiting {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(110)).await;
        tx.commit().await.unwrap();
        let (result, db) = pending.await.unwrap();
        assert!(matches!(result, Err(shared::AppError::Unauthorized)));
        let n: i64 = db
            .query_one(sql(
                "SELECT count(*) AS n FROM tracker_identity_deletion_operations",
                [],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap();
        assert_eq!(n, 0);
        db.close().await.unwrap();
        blocker.close().await.unwrap();
    })
    .await;
}
#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn explicit_issuer_binding_is_idempotent_and_immutable() {
    isolated(|db, _| async move {
        let owner = IdentityDeletionStore::new(&db);
        owner.bind_identity_legacy_issuer(ISSUER).await.unwrap();
        assert!(matches!(
            owner
                .bind_identity_legacy_issuer("https://other.example.test")
                .await,
            Err(shared::AppError::Conflict(_))
        ));
        assert!(
            owner
                .bind_identity_legacy_issuer("https://user:secret@example.test")
                .await
                .is_err()
        );
        db.close().await.unwrap();
    })
    .await;
}

#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn original_owner_deadline_rolls_back_freeze_and_release_inside_commit() {
    isolated(|mut db, url| async move {

        let cmd = request();
        IdentityDeletionStore::new(&db).identity_owner_freeze(&cmd, &window()).await.unwrap();
        // Compare stored operation/fence/receipt and the transactional epoch,
        // not nextval(): PostgreSQL sequences intentionally do not roll back.
        let before: serde_json::Value = db.query_one(Statement::from_string(DatabaseBackend::Postgres,"SELECT jsonb_build_object('operations',(SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY operation_id),'[]'::jsonb) FROM tracker_identity_deletion_operations t),'epoch',(SELECT to_jsonb(e) FROM tracker_identity_fence_epoch e WHERE singleton)) AS snapshot")).await.unwrap().unwrap().try_get("", "snapshot").unwrap();
        db.execute_unprepared("CREATE SEQUENCE qa_identity_commit_deadline; CREATE FUNCTION qa_identity_commit_wait() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM setval('qa_identity_commit_deadline',floor(extract(epoch FROM NEW.valid_until)*1000000)::bigint,true); PERFORM pg_advisory_xact_lock(714402216205::bigint); RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER a_qa_identity_commit_wait AFTER INSERT ON tracker_request_commit_fences DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION qa_identity_commit_wait();").await.unwrap();
        let owner_pid: i32 = db.query_one(Statement::from_string(DatabaseBackend::Postgres,
            "SELECT pg_backend_pid() AS pid")).await.unwrap().unwrap().try_get("", "pid").unwrap();
        for release in [false, true] {
            let blocker = connect(&url).await;
            let barrier = blocker.begin().await.unwrap();
            let blocker_pid: i32 = barrier.query_one(Statement::from_string(DatabaseBackend::Postgres,
                "SELECT pg_backend_pid() AS pid")).await.unwrap().unwrap().try_get("", "pid").unwrap();
            barrier.execute_unprepared("SELECT pg_advisory_xact_lock(714402216205::bigint)").await.unwrap();
            barrier.execute_unprepared("SELECT setval('qa_identity_commit_deadline',1,false)").await.unwrap();

            let r = if release { cmd.clone() } else { request() };
            let w = IdentityDeletionWindow::restricted(Instant::now()+Duration::from_secs(5), chrono::Utc::now().timestamp()+60);
            let pending = tokio::spawn(async move {
                let owner = IdentityDeletionStore::new(&db); let result = if release { owner.identity_owner_release(&r, &w).await.map(|_| ()) } else { owner.identity_owner_freeze(&r, &w).await.map(|_| ()) }; (result, db)
            });
            struct Abort(tokio::task::AbortHandle);
            impl Drop for Abort { fn drop(&mut self) { self.0.abort(); } }
            let _abort = Abort(pending.abort_handle());
            tokio::time::timeout(Duration::from_secs(4), async {
                loop {
                    let waiting: bool = barrier.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
                        "SELECT is_called AND $1=ANY(pg_blocking_pids($2)) AS waiting FROM qa_identity_commit_deadline",
                        [blocker_pid.into(), owner_pid.into()])).await.unwrap().unwrap().try_get("", "waiting").unwrap();
                    if waiting { break; }
                    tokio::task::yield_now().await;
                }
            }).await.expect("the actual owner must reach the deferred COMMIT barrier while current");
            let entered_current: bool = barrier.query_one(Statement::from_string(DatabaseBackend::Postgres,
                "SELECT is_called AND clock_timestamp()<to_timestamp(last_value::double precision/1000000) AS current FROM qa_identity_commit_deadline")).await.unwrap().unwrap().try_get("", "current").unwrap();
            assert!(entered_current, "must wait inside COMMIT before the bound original deadline");
            tokio::time::timeout(Duration::from_secs(6), async {
                loop {
                    let expired: bool = barrier.query_one(Statement::from_string(DatabaseBackend::Postgres,
                        "SELECT clock_timestamp()>=to_timestamp(last_value::double precision/1000000) AS expired FROM qa_identity_commit_deadline")).await.unwrap().unwrap().try_get("", "expired").unwrap();
                    if expired { break; }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.expect("the fixed DB deadline must expire without renewing the window");
            barrier.commit().await.unwrap();
            let (result, returned) = pending.await.unwrap(); db = returned;
            let result_code = match &result {
                Err(shared::AppError::Unauthorized) => 0_u8,
                Ok(()) => 1,
                Err(shared::AppError::Unavailable(_)) => 2,
                Err(shared::AppError::Conflict(_)) => 3,
                _ => 4,
            };
            let phase = if release { 10_u8 } else { 0_u8 };
            assert_eq!(phase + result_code, phase, "finite owner COMMIT outcome: freeze=0, release=10");
            assert!(matches!(result, Err(shared::AppError::Unauthorized)), "the released database clock must reject the expired COMMIT");
            let after: serde_json::Value = db.query_one(Statement::from_string(DatabaseBackend::Postgres,"SELECT jsonb_build_object('operations',(SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY operation_id),'[]'::jsonb) FROM tracker_identity_deletion_operations t),'epoch',(SELECT to_jsonb(e) FROM tracker_identity_fence_epoch e WHERE singleton)) AS snapshot")).await.unwrap().unwrap().try_get("", "snapshot").unwrap();
            assert_eq!(before, after, "failed freeze/release must preserve old fences, receipts, journal and epoch revision");
            let left: i64 = db.query_one(Statement::from_string(DatabaseBackend::Postgres,
                "SELECT count(*) AS n FROM tracker_request_commit_fences")).await.unwrap().unwrap().try_get("", "n").unwrap();
            assert_eq!(left, 0, "request clocks are transaction-local, never permanent owner fences");
            blocker.close().await.unwrap();
        }
        db.execute_unprepared("DROP TRIGGER a_qa_identity_commit_wait ON tracker_request_commit_fences; DROP FUNCTION qa_identity_commit_wait(); DROP SEQUENCE qa_identity_commit_deadline").await.unwrap();
        IdentityDeletionStore::new(&db).identity_owner_release(&cmd, &window()).await.unwrap();
        db.close().await.unwrap();
    }).await;
}
