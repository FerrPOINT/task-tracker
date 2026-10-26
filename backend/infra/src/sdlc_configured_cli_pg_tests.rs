//! Real owned PostgreSQL business ledger; identities are synthetic, not live Auth proof.
use super::*;

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn configured_cli_business_writes_need_current_assignment_but_no_native_lease() {
    const MIGRATION: &str = "m20261026_0000105_configured_cli";
    assert!(
        migration::Migrator::migrations()
            .iter()
            .any(|m| m.name() == MIGRATION),
        "integrated configured CLI QA requires registered 105; SQL leaf fallback is forbidden"
    );
    let mut f = Fixture::new().await;
    f.repo.config.verifier_subject.clear();
    let installed =
        f.db.query_one(statement(
            "SELECT to_regprocedure('sdlc_configured_cli_marker_gate()') IS NOT NULL
                AND EXISTS(SELECT 1 FROM seaql_migrations WHERE version=$1) AS installed",
            vec![MIGRATION.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "installed")
        .unwrap();
    assert!(
        installed,
        "ordinary Migrator must apply registered 105 and record its real ledger row"
    );
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
    let pm = actor(
        &subject,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    assert_eq!(pm.scopes.len(), 2);
    for scope in ["task-tracker:read", "task-tracker:write"] {
        let mut partial = pm.clone();
        partial.scopes.remove(scope);
        assert_eq!(
            f.repo.read(task, &partial).await.is_ok(),
            scope == "task-tracker:write",
            "missing {scope}"
        );
    }
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
    foreign.subject = "verifier".into();
    assert!(f.repo.read(task, &foreign).await.is_err());
    assert!(
        f.repo
            .execute_verified(
                task,
                &foreign,
                SdlcCommand::PublishQuestion(question),
                reservation.clone(),
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
    let answer = AnswerCommand {
        expected_question_version: 1,
        requirement_revision: 1,
        selected_option_ids: vec![],
        text: Some("Owner business answer".into()),
        comment: None,
        idempotency_key: "owner-answer".into(),
    };
    f.repo
        .execute_verified(
            task,
            &f.owner,
            SdlcCommand::Answer {
                question_id: state.questions[0].id,
                command: answer,
            },
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    let mut document = state.revisions[0].document.clone();
    document.goal.push_str("; owner answer incorporated");
    f.repo
        .execute_verified(
            task,
            &pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: fence.clone(),
                expected_requirement_revision: Some(1),
                document,
                idempotency_key: "final-revision".into(),
            }),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    let revised = f.repo.read(task, &pm).await.unwrap();
    let consent = SdlcCommand::Confirm {
        revision: 2,
        command: ConfirmCommand {
            content_hash: revised.revisions.last().unwrap().content_hash.clone(),
            idempotency_key: "owner-confirm".into(),
            expected_routing_policy_version: None,
        },
    };
    assert!(
        f.repo
            .execute_verified(task, &pm, consent.clone(), reservation.clone(), None)
            .await
            .is_err()
    );
    let mut stale_consent = consent.clone();
    if let SdlcCommand::Confirm { revision, .. } = &mut stale_consent {
        *revision = 1;
    }
    assert!(
        f.repo
            .execute_verified(task, &f.owner, stale_consent, reservation.clone(), None)
            .await
            .is_err()
    );
    for (field, value) in [
        ("stage", json!("Backlog")),
        ("owner_subject", json!(Uuid::new_v4().to_string())),
        ("content_hash", json!("b".repeat(64))),
    ] {
        let id = Uuid::new_v4();
        let hash = revised.revisions.last().unwrap().content_hash.clone();
        let mut payload = json!({"id":id,"task_id":task,"revision":2,"content_hash":hash,
            "owner_subject":f.owner.subject,"created_at":chrono::Utc::now(),"stage":"Draft"});
        payload[field] = value;
        let tx = f.db.begin().await.unwrap();
        assert!(
            tx.execute(statement(
                "INSERT INTO sdlc_confirmations(id,task_id,revision,content_hash,payload)
            VALUES($1,$2,2,$3,$4)",
                vec![id.into(), task.into(), hash.into(), payload.into()]
            ))
            .await
            .is_err(),
            "{field}"
        );
        tx.rollback().await.unwrap();
    }
    f.repo
        .execute_verified(task, &f.owner, consent.clone(), reservation.clone(), None)
        .await
        .unwrap();
    let confirmed = f.repo.read(task, &pm).await.unwrap();
    assert_eq!(confirmed.stage, Stage::Draft);
    assert_eq!(confirmed.status, Some(LifecycleStatus::InProgress));
    assert!(confirmed.evidence.is_empty());
    assert_eq!(
        confirmed.current_confirmation().unwrap().stage,
        Stage::Draft
    );
    assert_eq!(
        f.repo.confirmation_read(task, &pm, 2).await.unwrap().stage,
        Stage::Draft
    );
    assert_eq!(
        f.db.query_one(statement(
            // Ready is shared across stages; check the canonical pair, not a reverse status lookup.
            "SELECT m.stage||'/'||m.status AS projected FROM issues i
        JOIN sdlc_tasks t ON t.task_id=i.id
        JOIN sdlc_issue_status_mapping m ON m.status_id=i.status_id
            AND m.stage=t.state->>'stage' AND m.status=t.state->>'status' WHERE i.id=$1",
            vec![task.into()]
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "projected")
        .unwrap(),
        "Draft/InProgress"
    );
    assert_eq!(
        f.repo
            .execute_verified(task, &f.owner, consent, reservation.clone(), None)
            .await
            .unwrap()["stage"],
        "Draft"
    );
    // A direct publication without the authoritative completion ledger is refused.
    let tx = f.db.begin().await.unwrap();
    assert!(tx.execute(statement("UPDATE sdlc_tasks SET state=(state-'draft_submode')||'{\"stage\":\"Backlog\",\"status\":\"Ready\"}'::jsonb WHERE task_id=$1",vec![task.into()])).await.is_err());
    tx.rollback().await.unwrap();
    let command: domain::sdlc_configured_cli::CompleteConfiguredPm = parse(
        json!({"interaction_protocol":"configured-cli-v1",
        "fence":fence,"requirement_revision":2,"content_hash":confirmed.revisions.last().unwrap().content_hash,
        "namespace_id":2,"workflow_execution_ref":Uuid::new_v4(),"workflow_operation_key":"final-report",
        "workflow_result_version":4,"idempotency_key":"complete-configured"}),
    );
    // Controlled Workflow receipt, not a claim of real Workflow/provider execution.
    let receipt: domain::sdlc_configured_cli::ConfiguredWorkflowReceipt = parse(
        json!({"interaction_protocol":"configured-cli-v1",
        "execution_ref":command.workflow_execution_ref,"task":reservation.execution.key,"tracker_task_id":task,"namespace_id":2,
        "concrete_agent_ref":fence.agent_id,"assignment_ref":fence.assignment_id,"tracker_execution_id":fence.execution_id,
        "assignment_version":fence.assignment_version,"machine_subject":pm.subject,"operation_key":"final-report",
        "request_sha256":"a".repeat(64),"result_version":4,"requirements_revision":2,"requirements_sha256":command.content_hash,
        "owner_subject":confirmed.owner_subject,"confirmation_id":confirmed.current_confirmation().unwrap().id,"history_id":17,"status":"done"}),
    );
    let server_reader = actor("foreign", false, &["task-tracker:read"]);
    assert_ne!(
        server_reader.subject,
        f.repo.config.reservation_scheduler_subject
    );
    assert_ne!(server_reader.subject, f.repo.config.orchestrator_subject);
    assert_ne!(server_reader.subject, f.repo.config.verifier_subject);
    // Context and requirements GETs share repository.read; confirmation has its
    // own receipt reader. Neither may bypass current project membership.
    assert!(matches!(
        f.repo.read(task, &server_reader).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        f.repo.confirmation_read(task, &server_reader, 2).await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role)
        SELECT $1,id,'developer' FROM users WHERE central_sub='foreign'",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    let before_reader = f.snapshot().await;
    let context = f
        .repo
        .read(task, &server_reader)
        .await
        .unwrap()
        .context(&server_reader);
    assert_eq!(context.assignment, Some(reservation.assignment.clone()));
    assert_eq!(
        context.workflow_task_key.as_deref(),
        Some(reservation.execution.key.as_str())
    );
    let requirements = f
        .repo
        .read(task, &server_reader)
        .await
        .unwrap()
        .revisions
        .last()
        .cloned()
        .unwrap();
    assert_eq!(
        serde_json::to_value(&requirements).unwrap(),
        serde_json::to_value(confirmed.revisions.last().unwrap()).unwrap()
    );
    let consent = f
        .repo
        .confirmation_read(task, &server_reader, 2)
        .await
        .unwrap();
    assert_eq!(consent, *confirmed.current_confirmation().unwrap());
    let writes = [
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: Some(2),
            document: requirements.document.clone(),
            idempotency_key: "reader-must-not-publish".into(),
        }),
        SdlcCommand::Cancel {
            question_id: confirmed.questions[0].id,
            command: CancelQuestion {
                fence: fence.clone(),
                expected_question_version: confirmed.questions[0].version,
                idempotency_key: "reader-must-not-cancel".into(),
            },
        },
        SdlcCommand::Confirm {
            revision: 2,
            command: ConfirmCommand {
                content_hash: requirements.content_hash.clone(),
                idempotency_key: "reader-must-not-confirm".into(),
                expected_routing_policy_version: None,
            },
        },
    ];
    for write in writes {
        assert!(matches!(
            f.repo
                .execute_verified(task, &server_reader, write, reservation.clone(), None)
                .await,
            Err(AppError::Forbidden)
        ));
    }
    assert!(matches!(
        f.repo
            .complete_configured_pm(task, &server_reader, command.clone(), receipt.clone())
            .await,
        Err(AppError::Forbidden)
    ));
    let mut foreign_writer = server_reader.clone();
    foreign_writer.scopes.insert("task-tracker:write".into());
    assert!(matches!(
        f.repo.read(task, &foreign_writer).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        f.repo.confirmation_read(task, &foreign_writer, 2).await,
        Err(AppError::Forbidden)
    ));
    let mut stale_fence = command.clone();
    stale_fence.fence.agent_id = Uuid::new_v4();
    assert!(
        f.repo
            .complete_configured_pm(task, &pm, stale_fence, receipt.clone())
            .await
            .is_err()
    );
    assert_eq!(f.snapshot().await, before_reader);
    assert!(
        f.repo
            .complete_configured_pm(task, &f.owner, command.clone(), receipt.clone())
            .await
            .is_err()
    );
    let mut forged = receipt.clone();
    forged.assignment_version += 1;
    assert!(
        f.repo
            .complete_configured_pm(task, &pm, command.clone(), forged)
            .await
            .is_err()
    );
    for (field, value) in [
        ("status", json!("active")),
        ("requirements_sha256", json!("b".repeat(64))),
        ("confirmation_id", json!(Uuid::new_v4())),
        ("machine_subject", json!(Uuid::new_v4().to_string())),
    ] {
        let mut bad = serde_json::to_value(&receipt).unwrap();
        bad[field] = value;
        let bad = serde_json::from_value(bad).unwrap();
        assert!(
            f.repo
                .complete_configured_pm(task, &pm, command.clone(), bad)
                .await
                .is_err()
        );
    }
    assert_eq!(f.repo.read(task, &pm).await.unwrap().stage, Stage::Draft);
    assert_eq!(
        f.db.query_one(statement(
            "SELECT count(*) AS n FROM sdlc_configured_pm_completions",
            vec![]
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap(),
        0
    );
    let tx = f.db.begin().await.unwrap();
    let uncommitted = app::sdlc_configured_cli::completion_result(
        &confirmed,
        &command,
        receipt.clone(),
        reservation.owner_cas.version,
        Uuid::new_v4(),
        crate::sdlc::execution_lease::clock(&tx).await.unwrap(),
    )
    .unwrap();
    exec(&tx,"INSERT INTO sdlc_configured_pm_completions(completion_id,task_id,execution_id,confirmation_id,requirement_revision,content_hash,
        actor_subject,idempotency_key,payload_hash,command,workflow_receipt,result,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        vec![uncommitted.completion_id.into(),task.into(),fence.execution_id.into(),uncommitted.confirmation_id.into(),
            command.requirement_revision.into(),command.content_hash.clone().into(),pm.subject.clone().into(),command.idempotency_key.clone().into(),
            app::sdlc::canonical_hash(&command).unwrap().into(),json_value(&command).unwrap(),json_value(&receipt).unwrap(),
            json_value(&uncommitted).unwrap(),uncommitted.created_at.into()]).await.unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(f.repo.read(task, &pm).await.unwrap().stage, Stage::Draft);
    let (left, right) = tokio::join!(
        f.repo
            .complete_configured_pm(task, &pm, command.clone(), receipt.clone()),
        f.repo
            .complete_configured_pm(task, &pm, command.clone(), receipt.clone())
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.0, right.0);
    assert_ne!(left.1, right.1);
    let saved = f
        .repo
        .configured_pm_completion(task, &pm, Some(&command.idempotency_key))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.result, left.0);
    let mut changed = command.clone();
    changed.workflow_operation_key = "changed-report".into();
    assert!(
        f.repo
            .complete_configured_pm(task, &pm, changed, receipt.clone())
            .await
            .is_err()
    );
    for sql in [
        "UPDATE sdlc_configured_pm_completions SET payload_hash=repeat('0',64)",
        "DELETE FROM sdlc_configured_pm_completions",
    ] {
        let tx = f.db.begin().await.unwrap();
        assert!(tx.execute_unprepared(sql).await.is_err());
        tx.rollback().await.unwrap();
    }
    let after = f.repo.read(task, &pm).await.unwrap();
    assert_eq!(after.stage, Stage::Backlog);
    assert_eq!(after.status, Some(LifecycleStatus::Ready));
    assert_eq!(
        f.db.query_one(statement(
            "SELECT m.stage||'/'||m.status AS projected FROM issues i
        JOIN sdlc_tasks t ON t.task_id=i.id
        JOIN sdlc_issue_status_mapping m ON m.status_id=i.status_id
            AND m.stage=t.state->>'stage' AND m.status=t.state->>'status' WHERE i.id=$1",
            vec![task.into()]
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "projected")
        .unwrap(),
        "Backlog/Ready"
    );
    assert_eq!(after.confirmations, confirmed.confirmations);
    assert_eq!(
        f.repo.confirmation_read(task, &pm, 2).await.unwrap().stage,
        Stage::Draft
    );
    let mut expected = confirmed.clone();
    expected.stage = Stage::Backlog;
    expected.status = Some(LifecycleStatus::Ready);
    expected.draft_submode = None;
    assert_eq!(
        serde_json::to_value(expected).unwrap(),
        serde_json::to_value(&after).unwrap()
    );
    assert!(f.repo.config.single_task_project_ids.contains(&f.project));
    let before_scheduler = f.snapshot().await;
    assert!(
        f.repo
            .read_delivery_candidates(&f.reader, f.query(None, 100))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    for command in [
        LifecycleCommand::EnsureQueued {
            idempotency_key: "configured-no-analysis".into(),
        },
        LifecycleCommand::Reserve {
            expected_task_version: 1,
            idempotency_key: "configured-no-downstream".into(),
        },
    ] {
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.writer, command).await,
            Err(AppError::Conflict(_))
        ));
    }
    assert_eq!(f.snapshot().await, before_scheduler);
    for field in [
        "requirements",
        "owner",
        "assignment",
        "open_required_question",
        "timestamp",
    ] {
        let mut invalid = after.clone();
        match field {
            "requirements" => invalid.revisions[0].document.goal = "Changed".into(),
            "owner" => invalid.owner_subject = Uuid::new_v4().to_string(),
            "assignment" => invalid.assignment.as_mut().unwrap().version += 1,
            "open_required_question" => invalid.questions[0].state = QuestionState::Open,
            _ => invalid.confirmations[0].created_at += chrono::Duration::seconds(1),
        }
        let tx = f.db.begin().await.unwrap();
        assert!(
            tx.execute(statement(
                "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
                vec![task.into(), json_value(&invalid).unwrap()]
            ))
            .await
            .is_err(),
            "{field}"
        );
        tx.rollback().await.unwrap();
    }
    for table in [
        "sdlc_pm_execution_leases",
        "sdlc_pm_native_admissions",
        "sdlc_pm_lease_operations",
        "sdlc_analysis_intents",
        "sdlc_analysis_capacity_holds",
        "sdlc_delivery_heads",
        "sdlc_delivery_assignments",
        "sdlc_delivery_holds",
        "sdlc_delivery_operations",
        "sdlc_evidence",
        "sdlc_task_routing_snapshots",
        "sdlc_task_source_bindings",
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
