//! Actual serialized domain contracts and the fully migrated permit table.
//! These tests qualify the DB foundation; the coordinator RPC is a separate gate.
use domain::sdlc::{
    CreateDraftCommand, PmAssignment, Principal, SdlcConfig, SdlcRepository, TaskState,
};
use domain::sdlc_critical_pm_reservation::{
    CriticalPmRequirementsRef, CriticalPmReservationPurpose, CriticalPmReservationResult,
    CriticalPmReviewContext,
};
use domain::sdlc_metadata::MetadataInputRef;
use domain::sdlc_pm_draft::{
    OwnerCas, PmDraftAdmissionState, PmDraftBinding, PmDraftExecution, PmDraftReservation,
    PmDraftVariant,
};
use infra::sdlc::PostgresSdlcRepository;
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
    TransactionTrait,
};
use serde_json::{Value, json};
use std::future::Future;
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

fn critical_result(owner: &str, machine: &str) -> CriticalPmReservationResult {
    let task = Uuid::new_v4();
    let assignment = Uuid::new_v4();
    CriticalPmReservationResult {
        purpose: CriticalPmReservationPurpose::CriticalBusinessReview,
        reservation: PmDraftReservation {
            contract_version: 1,
            variant: PmDraftVariant::Reserved,
            binding: PmDraftBinding {
                tracker_instance_id: "identity-contract".into(),
                project_id: Uuid::new_v4(),
                task_id: task,
                root_task_id: task,
                owner_subject: owner.into(),
            },
            owner_cas: OwnerCas {
                expected_version: 1,
                version: 2,
            },
            assignment: PmAssignment {
                assignment_id: assignment,
                execution_id: Uuid::new_v4(),
                agent_id: Uuid::new_v4(),
                version: 2,
                machine_subject: machine.into(),
            },
            execution: PmDraftExecution {
                ordinal: "2".into(),
                key: "SDLC-2".into(),
            },
            input: MetadataInputRef {
                snapshot_ref: Uuid::new_v4(),
                sha256: "a".repeat(64),
            },
            assignment_operation_key: format!("pm-draft:{assignment}"),
            admission_state: PmDraftAdmissionState::Reserved,
            dispatch_allowed: false,
        },
        context: CriticalPmReviewContext {
            blocker_id: Uuid::new_v4(),
            blocker_sha256: "b".repeat(64),
            blocker_version: 1,
            requirements: CriticalPmRequirementsRef {
                revision: 1,
                content_hash: "c".repeat(64),
            },
        },
    }
}

#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn serialized_critical_reservation_result_extracts_owner_and_machine_from_real_row_contract()
{
    isolated(|db, _| async move {
        let owner = Uuid::new_v4().to_string();
        let machine = Uuid::new_v4().to_string();
        let result = critical_result(&owner, &machine);
        let serialized = serde_json::to_value(&result).unwrap();
        // Keep the complete typed wrapper; do not replace it with a hand-written
        // {assignment:{machine_subject}} fixture that misses `reservation`.
        let decoded: CriticalPmReservationResult = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(decoded.reservation.binding.owner_subject, owner);
        assert_eq!(decoded.reservation.assignment.machine_subject, machine);
        let rows = db.query_all(sql(
            "SELECT issuer,subject,resolved FROM tracker_identity_row_refs('sdlc_critical_pm_reviews',$1)",
            [json!({"result": serialized}).into()],
        )).await.unwrap();
        let mut subjects = Vec::new();
        for row in rows {
            assert!(row.try_get::<bool>("", "resolved").unwrap());
            assert_eq!(row.try_get::<String>("", "issuer").unwrap(), ISSUER);
            subjects.push(row.try_get::<String>("", "subject").unwrap());
        }
        subjects.sort();
        let mut expected = vec![owner, machine];
        expected.sort();
        assert_eq!(subjects, expected, "both persisted identities must survive the actual wrapper");
        db.close().await.unwrap();
    }).await;
}

async fn draft(db: &DatabaseConnection, url: &str) -> (PostgresSdlcRepository, TaskState) {
    let local_user = Uuid::new_v4();
    let owner = Uuid::new_v4().to_string();
    let project = Uuid::new_v4();
    db.execute(sql("INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)         VALUES($1,'permit-fixture@example.test','permit-fixture','Synthetic','!',$2,true)",
        [local_user.into(), owner.clone().into()])).await.unwrap();
    db.execute(sql("INSERT INTO projects(id,key,name,owner_id,default_board_id)         VALUES($1,'IDCT','Identity contract',$2,$3)",
        [project.into(), local_user.into(), Uuid::new_v4().into()])).await.unwrap();
    db.execute(sql("INSERT INTO boards(id,project_id,name,columns)         SELECT default_board_id,id,'Identity contract','[]' FROM projects WHERE id=$1",
        [project.into()])).await.unwrap();
    let repo = PostgresSdlcRepository::connect(
        url,
        SdlcConfig {
            instance_id: "identity-contract".into(),
            orchestrator_subject: Uuid::new_v4().to_string(),
            verifier_subject: Uuid::new_v4().to_string(),
            reservation_scheduler_subject: Uuid::new_v4().to_string(),
            single_task_project_ids: Default::default(),
        },
    )
    .await
    .unwrap();
    let actor = Principal {
        subject: owner,
        human_session: true,
        trusted_human: true,
        scopes: Default::default(),
    };
    let (created, replayed) = repo
        .create_draft(
            project,
            &actor,
            CreateDraftCommand {
                title: "Identity permit fixture".into(),
                description: "Synthetic persisted task".into(),
                idempotency_key: "identity-contract-draft".into(),
                flow: None,
            },
        )
        .await
        .unwrap();
    assert!(!replayed);
    let state: Value = db
        .query_one(sql(
            "SELECT state FROM sdlc_tasks WHERE task_id=$1",
            [created.task_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "state")
        .unwrap();
    (repo, serde_json::from_value(state).unwrap())
}

async fn insert_permit(db: &impl ConnectionTrait, next: &TaskState) -> Result<(), sea_orm::DbErr> {
    db.execute(sql("INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)         VALUES($1,pg_current_xact_id(),$2)",
        [next.task_id.into(), serde_json::to_value(next).unwrap().into()],
    )).await.map(|_| ())
}

async fn freeze_permit_subject(db: &DatabaseConnection, subject: &str) -> i64 {
    // Exercise the real foundation protocol, not an RPC that is not implemented yet.
    let operation = Uuid::new_v4();
    let inventory = Uuid::new_v4();
    let request = json!({"schema_version":1,"operation_id":operation,"inventory_revision":inventory,
        "identity":{"issuer":ISSUER,"subject":subject}});
    let tx = db.begin().await.unwrap();
    tx.execute_unprepared("SET LOCAL lock_timeout='8s'; SET LOCAL statement_timeout='8s';         SELECT pg_advisory_xact_lock(714402216002::bigint)").await.unwrap();
    assert_eq!(
        tx.execute_unprepared(
            "UPDATE tracker_identity_fence_epoch SET revision=revision+1 WHERE singleton"
        )
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    let count: i64 = tx.query_one(sql("SELECT count(*) AS total FROM sdlc_pm_state_write_permits p         WHERE EXISTS(SELECT 1 FROM tracker_identity_row_refs('sdlc_pm_state_write_permits',to_jsonb(p)) r             WHERE r.resolved AND r.issuer=$1 AND r.subject=$2)",
        [ISSUER.into(), subject.into()])).await.unwrap().unwrap().try_get("", "total").unwrap();
    tx.execute(sql("INSERT INTO tracker_identity_deletion_operations(operation_id,issuer,subject,inventory_revision,         request,state,fence,revision,reference_count) VALUES($1,$2,$3,$4,$5,'frozen',$6,         nextval('tracker_identity_reference_revision'),$7)",
        [operation.into(), ISSUER.into(), subject.into(), inventory.into(), request.into(), Uuid::new_v4().into(), count.into()],
    )).await.unwrap();
    tx.commit().await.unwrap();
    count
}

#[tokio::test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
async fn real_pm_state_permit_commit_is_inventoried_and_new_reference_is_rejected_after_freeze() {
    isolated(|db, url| async move {
        let (repo, mut next) = draft(&db, &url).await;
        // The reference exists only in this permit, not in a local profile or
        // the canonical task. Unconsumed permits are durable on the actual DDL.
        let target = Uuid::new_v4().to_string();
        next.owner_subject = target.clone();
        let tx = db.begin().await.unwrap();
        insert_permit(&tx, &next).await.unwrap();
        tx.commit().await.unwrap();
        let total: i64 = db
            .query_one(sql(
                "SELECT count(*) AS total FROM sdlc_pm_state_write_permits",
                [],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "total")
            .unwrap();
        assert_eq!(
            total, 1,
            "the real schema permits a committed residual journal"
        );
        assert_eq!(freeze_permit_subject(&db, &target).await, 1);
        let tx = db.begin().await.unwrap();
        let denied = insert_permit(&tx, &next)
            .await
            .expect_err("frozen owner ref must be rejected");
        assert!(denied.to_string().contains("identity_deletion_fenced"));
        tx.rollback().await.unwrap();
        let total: i64 = db
            .query_one(sql(
                "SELECT count(*) AS total FROM sdlc_pm_state_write_permits",
                [],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "total")
            .unwrap();
        assert_eq!(total, 1, "rejected transaction cannot append a permit");
        next.owner_subject = Uuid::new_v4().to_string();
        let tx = db.begin().await.unwrap();
        insert_permit(&tx, &next).await.unwrap();
        tx.commit().await.unwrap();
        drop(repo);
        db.close().await.unwrap();
    })
    .await;
}
