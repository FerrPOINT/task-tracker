//! Real SQL barrier/drain classifier using the focused repository fixture.
//! Terminal rows here model already accepted receipts. Full receipt authenticity
//! and full migration rehearsal remain separate server/integrated gates.
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use shared::{space_access::*, space_lifecycle::*};
use uuid::Uuid;
fn id(n: u64) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
fn command(space: SpaceRef, slug: &str) -> SpaceOwnerCommandV2 {
    SpaceOwnerCommandV2 {
        schema_version: 2,
        space,
        participant_instance_id: id(99),
        operation_id: Uuid::new_v4(),
        revision: 11,
        desired_state: DesiredSpaceState::Active,
        metadata: SpaceMetadata {
            slug: slug.into(),
            name: format!("Пространство {slug}"),
            description: "Synthetic contract fixture".into(),
            responsible_subject: id(30).to_string(),
        },
    }
}
async fn start(
    db: &DatabaseConnection,
    task: Uuid,
    assignment: Uuid,
    execution: Uuid,
    spaces: &[SpaceRef],
    required: Option<&SpaceRef>,
    proof: bool,
) -> Result<(), sea_orm::DbErr> {
    let tx = db.begin().await?;
    if proof {
        let identity = CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: id(30).to_string(),
        };
        let values = [
            (
                "pdlc.tracker.execution_spaces",
                serde_json::to_string(spaces).unwrap(),
            ),
            (
                "pdlc.tracker.execution_required_wiki_space",
                serde_json::to_string(&required).unwrap(),
            ),
            (
                "pdlc.tracker.execution_actor",
                serde_json::to_string(&identity).unwrap(),
            ),
            (
                "pdlc.tracker.admission_valid_until",
                (chrono::Utc::now().timestamp() + 2).to_string(),
            ),
        ];
        for (key, value) in values {
            tx.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config($1,$2,true)",
                [key.into(), value.into()],
            ))
            .await?;
        }
    }
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES($1,1,$2)",
        [
            task.into(),
            serde_json::json!({"assignment_id":assignment,"execution_id":execution}).into(),
        ],
    ))
    .await?;
    tx.commit().await?;
    Ok(())
}
async fn complete(
    db: &DatabaseConnection,
    task: Uuid,
    assignment: Uuid,
    execution: Uuid,
) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO sdlc_pm_completions(task_id,assignment_id,execution_id,assignment_version) VALUES($1,$2,$3,1)",[task.into(),assignment.into(),execution.into()])).await?;
    Ok(())
}
pub async fn verify(db: &DatabaseConnection) {
    let now = chrono::Utc::now().timestamp();
    let authority = SpaceOwnerAuthority::confirmed(
        CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: "registered-admin-owner".into(),
        },
        AdmissionValidity::confirmed(now, now + 60, i64::MAX).unwrap(),
    )
    .unwrap();
    let a = SpaceRef {
        registry_instance_id: id(10),
        space_id: id(11),
    };
    let b = SpaceRef {
        space_id: id(12),
        ..a.clone()
    };
    let c = SpaceRef {
        space_id: id(13),
        ..a.clone()
    };
    let mut ca = command(a.clone(), "alpha");
    let mut cb = command(b.clone(), "beta");
    for command in [&ca, &cb] {
        assert!(
            infra::space_owner::apply(db, command, &authority)
                .await
                .unwrap()
                .local_resource
                .is_none()
        );
    }
    let mut standalone = command(c.clone(), "standalone");
    standalone.revision = 1;
    let ack = infra::space_owner::apply(db, &standalone, &authority)
        .await
        .unwrap();
    assert!(ack.drained);
    assert!(ack.local_resource.is_none());
    let count:i64=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT count(*) AS n FROM tracker_project_spaces WHERE registry_instance_id=$1 AND space_id=$2",[c.registry_instance_id.into(),c.space_id.into()])).await.unwrap().unwrap().try_get("","n").unwrap();
    assert_eq!(count, 0);
    standalone.operation_id = Uuid::new_v4();
    standalone.revision = 2;
    standalone.desired_state = DesiredSpaceState::Archived;
    assert!(
        infra::space_owner::apply(db, &standalone, &authority)
            .await
            .unwrap()
            .drained
    );
    let mut wrong = standalone.clone();
    wrong.metadata.name = "Другой замысел".into();
    assert!(
        infra::space_owner::apply(db, &wrong, &authority)
            .await
            .is_err()
    );
    // Keep the canonical project in both spaces; owner lifecycle never rewrites this set.
    db.execute_unprepared("INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011')").await.unwrap();
    let project = shared::ProjectId::from_uuid(id(1));
    let before = infra::project_spaces::get(db, project).await.unwrap();
    for task in [id(201), id(202), id(203)] {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO sdlc_tasks(task_id,project_id) VALUES($1,$2)",
            [task.into(), id(1).into()],
        ))
        .await
        .unwrap();
    }
    assert!(
        start(db, id(201), id(501), id(601), &[b.clone()], None, false)
            .await
            .is_err()
    );
    start(db, id(201), id(501), id(601), &[b.clone()], None, true)
        .await
        .unwrap();
    ca.operation_id = Uuid::new_v4();
    ca.revision = 12;
    ca.desired_state = DesiredSpaceState::Archived;
    assert!(
        infra::space_owner::apply(db, &ca, &authority)
            .await
            .unwrap()
            .drained,
        "B-only work must not block archive of A"
    );
    let retained = domain::project_spaces::ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: before.revision,
        spaces: vec![a.clone(), b.clone()],
    };
    let kept = infra::project_spaces::replace(
        db,
        project,
        &retained,
        &super::permit(shared::UserId::from_uuid(id(30)), &[b.clone()], 60),
    )
    .await
    .unwrap();
    assert_eq!(
        kept.spaces, before.spaces,
        "archived A is retained without being reinserted"
    );
    assert_eq!(
        kept.revision, before.revision,
        "a no-op is not a new link revision"
    );
    assert!(
        infra::project_spaces::replace(
            db,
            project,
            &retained,
            &super::permit(shared::UserId::from_uuid(id(30)), &[a.clone()], 60)
        )
        .await
        .is_err(),
        "A-only actor cannot borrow hidden active B for relationship management"
    );
    assert!(
        start(db, id(203), id(503), id(603), &[a.clone()], None, true)
            .await
            .is_err(),
        "A-only actor cannot borrow active B"
    );
    assert!(
        start(
            db,
            id(203),
            id(503),
            id(603),
            &[a.clone(), b.clone()],
            Some(&a),
            true
        )
        .await
        .is_err(),
        "explicit Wiki context A remains read-only despite B"
    );
    start(db, id(202), id(502), id(602), &[b.clone()], None, true)
        .await
        .unwrap();
    cb.operation_id = Uuid::new_v4();
    cb.revision = 12;
    cb.desired_state = DesiredSpaceState::Archived;
    let pending = infra::space_owner::apply(db, &cb, &authority)
        .await
        .unwrap();
    assert!(!pending.drained);
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    assert!(
        !infra::space_owner::apply(db, &cb, &authority)
            .await
            .unwrap()
            .drained,
        "expired initial grants do not prove stopped executions"
    );
    let mut premature = cb.clone();
    premature.operation_id = Uuid::new_v4();
    premature.revision = 13;
    premature.desired_state = DesiredSpaceState::Active;
    assert!(
        infra::space_owner::apply(db, &premature, &authority)
            .await
            .is_err()
    );
    assert!(
        start(db, id(203), id(503), id(603), &[b.clone()], None, true)
            .await
            .is_err()
    );
    complete(db, id(202), id(502), id(602)).await.unwrap();
    complete(db, id(201), id(501), id(999)).await.unwrap();
    assert!(
        !infra::space_owner::apply(db, &cb, &authority)
            .await
            .unwrap()
            .drained,
        "wrong execution receipt must not release the original assignment"
    );
    complete(db, id(201), id(501), id(601)).await.unwrap();
    let archived = infra::space_owner::apply(db, &cb, &authority)
        .await
        .unwrap();
    assert!(archived.drained);
    let after = infra::project_spaces::get(db, project).await.unwrap();
    assert_eq!(after.spaces, before.spaces);
    assert_eq!(after.revision, before.revision);
    let mut restore = cb.clone();
    restore.operation_id = Uuid::new_v4();
    restore.revision = 13;
    restore.desired_state = DesiredSpaceState::Active;
    assert!(
        infra::space_owner::apply(db, &restore, &authority)
            .await
            .unwrap()
            .drained
    );
    assert_eq!(
        infra::space_owner::apply(db, &cb, &authority)
            .await
            .unwrap(),
        archived,
        "old completed receipt must remain replayable"
    );
    let state: String = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT state FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2",
            [b.registry_instance_id.into(), b.space_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "state")
        .unwrap();
    assert_eq!(
        state, "active",
        "late archived ACK must not roll back a newer restore"
    );
}
