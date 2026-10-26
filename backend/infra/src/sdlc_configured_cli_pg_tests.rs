//! Real owned PostgreSQL business ledger; identities are synthetic, not live Auth proof.
use super::*;

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn configured_cli_business_writes_need_current_assignment_but_no_native_lease() {
    let f = Fixture::new().await;
    let installed =
        f.db.query_one(statement(
            "SELECT to_regprocedure('sdlc_configured_cli_marker_gate()') IS NOT NULL AS installed",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "installed")
        .unwrap();
    // The integrator registers 105; this also permits scoped leaf qualification
    // before the common registry edit, without fabricating a migration ledger row.
    if !installed {
        f.db.execute_unprepared(include_str!("../../migration/src/configured_cli.sql"))
            .await
            .unwrap();
    }
    let subject = Uuid::new_v4().to_string();
    let local = Uuid::new_v4();
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
        vec![local.into(), format!("{local}@example.test").into(), subject.clone().into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![f.project.into(), local.into()],
    )
    .await
    .unwrap();
    let command = CreateDraftCommand {
        interaction_protocol: Some(InteractionProtocol::ConfiguredCliV1),
        flow: Some(DraftFlow::SingleTaskV1),
        title: "Configured PM".into(),
        description: "Synthetic business input".into(),
        idempotency_key: "configured-create".into(),
    };
    let (created, replay) = f
        .repo
        .create_draft(f.project, &f.owner, command.clone())
        .await
        .unwrap();
    assert!(!replay);
    let task = created.task_id;
    let (same, replay) = f
        .repo
        .create_draft(f.project, &f.owner, command)
        .await
        .unwrap();
    assert!(replay);
    assert_eq!(created, same);
    let reserve = parse(
        json!({"interaction_protocol":"configured-cli-v1","machine_subject":subject,
        "requested_agent_id":f.roles.project_manager.agent_id,"expected_owner_version":0,
        "expected_assignment_version":null,"idempotency_key":"configured-reserve"}),
    );
    let (reservation, _) = f
        .repo
        .reserve_pm_draft(task, &f.owner, reserve)
        .await
        .unwrap();
    let mut pm = actor(
        &subject,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    pm.scopes.insert(format!(
        "task-tracker:sdlc:agent:{}",
        reservation.assignment.agent_id
    ));
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    let revision: PublishRevision = parse(
        json!({"fence":fence,"expected_requirement_revision":null,
        "idempotency_key":"configured-revision","document":{"goal":"Goal","scope":["Scope"],"exclusions":[],
        "scenarios":["Scenario"],"acceptance_criteria":["Acceptance"],"constraints":[],"dependencies":[],
        "assumptions":[],"checklist":["Check"],"prerequisites":["Prerequisite"]}}),
    );
    assert!(matches!(
        f.repo
            .prepare_native_command(task, &pm, &SdlcCommand::PublishRevision(revision.clone()))
            .await
            .unwrap(),
        domain::sdlc_native_admission::CommandAdmission::MachineLease(_)
    ));
    let saved = f
        .repo
        .execute_verified(
            task,
            &pm,
            SdlcCommand::PublishRevision(revision.clone()),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        saved,
        f.repo
            .execute_verified(
                task,
                &pm,
                SdlcCommand::PublishRevision(revision.clone()),
                reservation.clone(),
                None
            )
            .await
            .unwrap()
    );
    let mut changed = revision;
    changed.document.goal = "Changed payload".into();
    assert!(
        f.repo
            .execute_verified(
                task,
                &pm,
                SdlcCommand::PublishRevision(changed),
                reservation.clone(),
                None
            )
            .await
            .is_err()
    );
    let question: PublishQuestion = parse(
        json!({"fence":fence,"request_id":Uuid::new_v4(),"question_id":Uuid::new_v4(),
        "expected_question_version":null,"requirement_revision":1,"requirement_reference":null,
        "text":"Business question","rationale":"Business choice","required":true,"mode":"text","options":[],
        "recommended_option_id":null,"idempotency_key":"configured-question"}),
    );
    f.repo
        .execute_verified(
            task,
            &pm,
            SdlcCommand::PublishQuestion(question.clone()),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    let state = f.repo.read(task, &pm).await.unwrap();
    assert_eq!(
        state.retained_fields["workflow_task_key"],
        json!(reservation.execution.key)
    );
    assert!(state.questions[0].checkpoint_id.is_none());
    assert!(!state.context(&pm).permissions.can_confirm);
    let mut foreign = pm.clone();
    foreign
        .scopes
        .retain(|s| !s.starts_with("task-tracker:sdlc:agent:"));
    foreign
        .scopes
        .insert(format!("task-tracker:sdlc:agent:{}", Uuid::new_v4()));
    assert!(f.repo.read(task, &foreign).await.is_err());
    assert!(
        f.repo
            .execute_verified(
                task,
                &foreign,
                SdlcCommand::PublishQuestion(question),
                reservation,
                None
            )
            .await
            .is_err()
    );
    let tx = f.db.begin().await.unwrap();
    assert!(
        tx.execute(statement(
            "UPDATE sdlc_tasks SET state=state-'interaction_protocol' WHERE task_id=$1",
            vec![task.into()]
        ))
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    for table in [
        "sdlc_pm_execution_leases",
        "sdlc_pm_native_admissions",
        "sdlc_pm_lease_operations",
        "sdlc_analysis_intents",
    ] {
        let sql = format!("SELECT count(*) AS n FROM {table}");
        assert_eq!(
            f.db.query_one(statement(&sql, vec![]))
                .await
                .unwrap()
                .unwrap()
                .try_get::<i64>("", "n")
                .unwrap(),
            0
        );
    }
}
