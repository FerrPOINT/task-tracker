//! Owned synthetic PostgreSQL only. Runtime/Workflow observations are not live evidence.
use super::*;
use domain::sdlc_configured_cli::{
    CompleteConfiguredPm, ConfiguredPmCompletionReceipt, ConfiguredWorkflowReceipt,
};
use domain::sdlc_discussion::DiscussionRepository;

const PRE_START: &str = "m20261028_0000110_configured_checkpoint_absence";
const PRE_AUTO_START: &str = "m20261029_0000111_configured_analysis_start";

async fn predecessor_boundary(f: &Fixture, tail: &str) -> Json {
    assert!(matches!(tail, PRE_START | PRE_AUTO_START));
    let migrations = migration::Migrator::migrations();
    let end = migrations.iter().position(|m| m.name() == tail).unwrap() + 1;
    let mut expected: Vec<_> = migrations[..end]
        .iter()
        .map(|m| m.name().to_owned())
        .collect();
    expected.sort();
    let actual: Vec<String> =
        f.db.query_all(statement(
            "SELECT version FROM seaql_migrations ORDER BY version",
            vec![],
        ))
        .await
        .unwrap()
        .iter()
        .map(|row| row.try_get("", "version").unwrap())
        .collect();
    assert_eq!(
        actual, expected,
        "historical DML requires the exact predecessor ledger"
    );
    let row =
        f.db.query_one(statement(
            "SELECT current_setting('session_replication_role')='origin' AS origin,
        bool_and(t.tgenabled IN ('O','A')) AS enabled,
        jsonb_agg(jsonb_build_object('table',c.relname,'trigger',t.tgname,'enabled',t.tgenabled,
            'definition',pg_get_triggerdef(t.oid)) ORDER BY c.relname,t.tgname) AS triggers
        FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid
        JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public'",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(row.try_get::<bool>("", "origin").unwrap());
    assert!(
        row.try_get::<bool>("", "enabled").unwrap(),
        "every historical guard must remain enabled"
    );
    row.try_get("", "triggers").unwrap()
}

async fn protected_history(
    f: &Fixture,
    names: &[String],
) -> std::collections::BTreeMap<String, Json> {
    let mut history = std::collections::BTreeMap::new();
    for name in names {
        // Same catalog-only identifier boundary as the existing populated-upgrade fixture.
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        );
        let sql = format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]') AS rows FROM {name} t"
        );
        let rows =
            f.db.query_one(statement(&sql, vec![]))
                .await
                .unwrap()
                .unwrap()
                .try_get("", "rows")
                .unwrap();
        history.insert(name.clone(), rows);
    }
    history
}

async fn protected_tables(f: &Fixture) -> Vec<String> {
    f.db.query_all(statement(r"SELECT tablename FROM pg_tables WHERE schemaname='public'
        AND (tablename LIKE 'sdlc\_%' ESCAPE '\' OR tablename IN ('issues','issue_status_history','comments')) ORDER BY tablename", vec![]))
        .await.unwrap().iter().map(|row| row.try_get("", "tablename").unwrap()).collect()
}

fn with_completion_transaction_column(
    mut before: std::collections::BTreeMap<String, Json>,
) -> std::collections::BTreeMap<String, Json> {
    let rows = before
        .get_mut("sdlc_configured_pm_completions")
        .unwrap()
        .as_array_mut()
        .unwrap();
    assert!(
        !rows.is_empty(),
        "upgrade must preserve populated completion history"
    );
    for row in rows {
        assert!(
            row.as_object_mut()
                .unwrap()
                .insert("transaction_id".into(), Json::Null)
                .is_none()
        );
    }
    before
}

async fn upgrade_preserving_predecessor(
    f: &Fixture,
    names: &[String],
    before: std::collections::BTreeMap<String, Json>,
) {
    let expected = with_completion_transaction_column(before);
    let old_ledger: Json =
        f.db.query_one(statement(
            "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS rows FROM seaql_migrations m",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "rows")
        .unwrap();
    migration::Migrator::up(&f.db, None).await.unwrap();
    let actual = protected_history(f, names).await;
    assert!(
        actual["sdlc_configured_pm_completions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row.get("transaction_id") == Some(&Json::Null))
    );
    assert_eq!(actual, expected);
    let mut expected_ledger: Vec<_> = migration::Migrator::migrations()
        .iter()
        .map(|m| m.name().to_owned())
        .collect();
    expected_ledger.sort();
    let actual_ledger: Vec<String> =
        f.db.query_all(statement(
            "SELECT version FROM seaql_migrations ORDER BY version",
            vec![],
        ))
        .await
        .unwrap()
        .iter()
        .map(|row| row.try_get("", "version").unwrap())
        .collect();
    assert_eq!(actual_ledger, expected_ledger);
    let ledger: Json =
        f.db.query_one(statement(
            "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS rows FROM seaql_migrations m",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "rows")
        .unwrap();
    for row in old_ledger.as_array().unwrap() {
        assert!(
            ledger.as_array().unwrap().contains(row),
            "original migration row changed: {row}"
        );
    }
    let added: Vec<_> = protected_tables(f)
        .await
        .into_iter()
        .filter(|name| !names.contains(name))
        .collect();
    assert!(
        protected_history(f, &added)
            .await
            .values()
            .all(|rows| rows.as_array().unwrap().is_empty()),
        "upgrade must not fabricate new work or authority"
    );
}

// Synthetic predecessor data, never runtime/Workflow acceptance. The create/reserve,
// persist_result/finish and comment DML are unchanged from f98faed9. Only the retired
// confirmation/completion projections are explicit here, under exact110/111 guards.
async fn predecessor_pm(
    f: &Fixture,
    key: &str,
    tail: &str,
) -> (Uuid, ConfiguredPmCompletionReceipt) {
    let guards = predecessor_boundary(f, tail).await;
    let task = f.draft(key, Some(DraftFlow::SingleTaskV1)).await;
    let (reservation, pm) = crate::sdlc::configured_test_support::assigned_pm(
        &f.repo,
        task,
        &f.owner,
        f.roles.project_manager.agent_id,
    )
    .await;
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    let document: RequirementsDocument = parse(json!({
        "goal":"Synthetic historical configured PM publication","scope":["Preserve the original task"],
        "exclusions":["No live acceptance"],"scenarios":["Upgrade without executing old work"],
        "acceptance_criteria":["Saved business facts remain unchanged"],"constraints":[],"dependencies":[],
        "assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]}));
    app::sdlc::validate_document(&document).unwrap();
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    let tx = f.db.begin().await.unwrap();
    let mut state = f.repo.load(&tx, task, &pm).await.unwrap();
    assert_eq!(state.stage, Stage::Draft);
    assert_eq!(state.status, Some(LifecycleStatus::Ready));
    assert_eq!(state.assignment.as_ref(), Some(&reservation.assignment));
    assert!(
        state.revisions.is_empty() && state.questions.is_empty() && state.confirmations.is_empty()
    );
    let revision = RequirementsRevision {
        revision: 1,
        content_hash: hash.clone(),
        document: document.clone(),
        author_subject: pm.subject.clone(),
        created_at: crate::sdlc::execution_lease::clock(&tx).await.unwrap(),
    };
    state.revisions.push(revision.clone());
    state.confirmation_revision = Some(1);
    state.draft_submode = Some(DraftSubmode::AwaitingConfirmation);
    let command = SdlcCommand::PublishRevision(PublishRevision {
        fence: fence.clone(),
        expected_requirement_revision: None,
        document,
        idempotency_key: "historical:requirements".into(),
    });
    let result = json!(revision);
    let event = f
        .repo
        .persist_result(&tx, task, &command, &result, &state)
        .await
        .unwrap();
    PostgresSdlcRepository::finish(
        &tx,
        task,
        &pm,
        command.key(),
        &app::sdlc::command_hash(&command).unwrap(),
        &result,
        event,
        &state,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let tx = f.db.begin().await.unwrap();
    let mut state = f.repo.load(&tx, task, &f.owner).await.unwrap();
    assert!(state.owner(&f.owner) && state.ready());
    assert_eq!(state.current_revision(), Some(1));
    assert_eq!(state.revisions.last().unwrap().content_hash, hash);
    let consent = Confirmation {
        id: Uuid::new_v4(),
        task_id: task,
        revision: 1,
        content_hash: hash.clone(),
        owner_subject: f.owner.subject.clone(),
        created_at: crate::sdlc::execution_lease::clock(&tx).await.unwrap(),
        stage: Stage::Draft,
    };
    state.confirmations.push(consent.clone());
    // f98 confirmation, not an agent StageStart operation: old consent set InProgress.
    state.status = Some(LifecycleStatus::InProgress);
    let command = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: hash.clone(),
            idempotency_key: "historical:consent".into(),
            expected_routing_policy_version: None,
        },
    };
    let result = json!(consent);
    let event = f
        .repo
        .persist_result(&tx, task, &command, &result, &state)
        .await
        .unwrap();
    PostgresSdlcRepository::finish(
        &tx,
        task,
        &f.owner,
        command.key(),
        &app::sdlc::command_hash(&command).unwrap(),
        &result,
        event,
        &state,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    f.repo
        .publish_stage_comment(
            task,
            &pm,
            domain::sdlc_discussion::PublishStageComment {
                fence: fence.clone(),
                body: "Synthetic historical PM final comment; no live Workflow observation".into(),
                idempotency_key: "historical:final-comment".into(),
            },
        )
        .await
        .unwrap();
    let command = CompleteConfiguredPm {
        interaction_protocol: InteractionProtocol::ConfiguredCliV1,
        fence: fence.clone(),
        requirement_revision: 1,
        content_hash: hash.clone(),
        namespace_id: 1,
        workflow_execution_ref: Uuid::new_v4(),
        workflow_operation_key: "historical:workflow-final".into(),
        workflow_result_version: 4,
        idempotency_key: "historical:completion".into(),
    };
    let workflow: ConfiguredWorkflowReceipt = parse(json!({
        "interaction_protocol":"configured-cli-v1","execution_ref":command.workflow_execution_ref,
        "task":reservation.execution.key,"tracker_task_id":task,"namespace_id":command.namespace_id,
        "concrete_agent_ref":fence.agent_id,"assignment_ref":fence.assignment_id,"tracker_execution_id":fence.execution_id,
        "assignment_version":fence.assignment_version,"machine_subject":pm.subject,"operation_key":command.workflow_operation_key,
        "request_sha256":"a".repeat(64),"result_version":command.workflow_result_version,"requirements_revision":1,
        "requirements_sha256":hash,"owner_subject":f.owner.subject,"confirmation_id":consent.id,"history_id":17,"status":"done"}));
    let tx = f.db.begin().await.unwrap();
    let state = f.repo.load(&tx, task, &pm).await.unwrap();
    app::sdlc_configured_cli::validate_completion(&state, &pm, &command).unwrap();
    crate::sdlc::discussion::require_stage_comment(
        &tx,
        &state,
        &pm.subject,
        fence.assignment_id,
        fence.execution_id,
    )
    .await
    .unwrap();
    let at = crate::sdlc::execution_lease::clock(&tx).await.unwrap();
    let mut completed = app::sdlc_configured_cli::completion_result(
        &state,
        &command,
        workflow,
        reservation.owner_cas.version,
        Uuid::new_v4(),
        at,
    )
    .unwrap();
    // The original105 SQL accepts exactly Backlog/Ready, not the new115 receipt pair.
    completed.stage = Stage::Backlog;
    completed.status = LifecycleStatus::Ready;
    exec(&tx,"INSERT INTO sdlc_configured_pm_completions(completion_id,task_id,execution_id,confirmation_id,requirement_revision,content_hash,
        actor_subject,idempotency_key,payload_hash,command,workflow_receipt,result,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        vec![completed.completion_id.into(),task.into(),fence.execution_id.into(),consent.id.into(),1i64.into(),hash.into(),
            pm.subject.clone().into(),command.idempotency_key.clone().into(),app::sdlc::canonical_hash(&command).unwrap().into(),
            json_value(&command).unwrap(),json_value(&completed.workflow_receipt).unwrap(),json_value(&completed).unwrap(),at.into()]).await.unwrap();
    let mut next = state.clone();
    next.stage = Stage::Backlog;
    next.status = Some(LifecycleStatus::Ready);
    next.draft_submode = None;
    exec(&tx,"INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state) VALUES($1,pg_current_xact_id(),$2)",vec![task.into(),json_value(&next).unwrap()]).await.unwrap();
    exec(
        &tx,
        "UPDATE sdlc_tasks SET state=$2,pm_admission_state='completed' WHERE task_id=$1",
        vec![task.into(), json_value(&next).unwrap()],
    )
    .await
    .unwrap();
    let status =
        crate::sdlc::lifecycle::mapped_issue_status(&tx, Stage::Backlog, LifecycleStatus::Ready)
            .await
            .unwrap();
    crate::sdlc::lifecycle::project_issue_status(&tx, task, &pm, status)
        .await
        .unwrap();
    exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'pm.completed',$3)",
        vec![completed.completion_id.into(),task.into(),json!({"contract_version":1,"interaction_protocol":"configured-cli-v1",
            "tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,"task_id":task,"root_task_id":task,
            "owner_subject":state.owner_subject,"stage":"Backlog","requirement_revision":1,"result":completed}).into()]).await.unwrap();
    tx.commit().await.unwrap();
    f.assert_backlog_without_analysis(task).await;
    assert_eq!(predecessor_boundary(f, tail).await, guards);
    (task, completed)
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn forward_start_migration_preserves_filled_110_history_without_auto_start() {
    Box::pin(async {
        let migrations = migration::Migrator::migrations();
        let checkpoint = migrations
            .iter()
            .position(|migration| migration.name() == "m20261029_0000111_configured_analysis_start")
            .unwrap();
        let f = Box::pin(Fixture::new_with_migration_steps(Some(checkpoint as u32))).await;
        let (pending, completed) = Box::pin(predecessor_pm(&f, "upgrade-backlog", PRE_START)).await;
        let before = f.snapshot().await;
        let names = protected_tables(&f).await;
        let history = protected_history(&f, &names).await;
        // Schema110 supports completed PM history, not configured delivery operations.
        assert!(
            history["sdlc_delivery_operations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            history["sdlc_configured_pm_completions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            history["sdlc_configured_pm_completions"][0]["task_id"],
            json!(pending)
        );
        upgrade_preserving_predecessor(&f, &names, history).await;
        assert_eq!(f.snapshot().await, before);
        f.assert_backlog_without_analysis(pending).await;
        let preserved = protected_history(&f, &names).await;
        let saved = f
            .repo
            .configured_pm_completion(pending, &f.owner, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(json!(saved.result), json!(completed));
        assert!(
            f.repo
                .read_delivery_candidates(&f.reader, f.query(None, 100))
                .await
                .unwrap()
                .items
                .is_empty()
        );
        let cmd = command(&f, pending, "after-upgrade").await;
        assert!(matches!(
            f.repo.lifecycle_command(pending, &f.owner, cmd).await,
            Err(AppError::Conflict(_))
        ));
        assert!(matches!(
            f.repo
                .lifecycle_operation(pending, &f.owner, "after-upgrade")
                .await,
            Err(AppError::NotFound(_))
        ));
        assert_eq!(protected_history(&f, &names).await, preserved);
        assert_eq!(f.snapshot().await, before);
    })
    .await;
}

async fn command(f: &Fixture, task: Uuid, key: &str) -> LifecycleCommand {
    let completion = f
        .repo
        .configured_pm_completion(task, &f.owner, None)
        .await
        .unwrap()
        .unwrap();
    LifecycleCommand::StartAnalysis {
        pm_completion_id: completion.result.completion_id,
        idempotency_key: key.into(),
    }
}

#[tokio::test]
#[serial_test::serial]
#[ignore = "requires isolated process and fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn configured_scheduler_reserve_and_bind_keep_native_owner_and_space_guards() {
    use app::native_space_access::{NativeRequest, with_request};
    use axum::{
        Json as HttpJson, Router,
        extract::State,
        http::HeaderMap,
        routing::{get, post},
    };
    use std::sync::RwLock;

    // Seed through the existing business fixture before enabling the serving policies.
    // Run this selector alone: the real central bridge keeps its configuration per process.
    assert!(std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_none());
    assert!(std::env::var_os("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED").is_none());
    let mut f = Box::pin(Fixture::new()).await;
    // Create fresh identities and membership; existing identity aliases are immutable.
    let owner_identity = crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let scheduler_identity =
        crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let owner = Uuid::parse_str(&owner_identity.subject).unwrap();
    let scheduler = Uuid::parse_str(&scheduler_identity.subject).unwrap();
    f.owner = actor(&owner_identity.subject, true, &[]);
    f.writer = actor(&scheduler_identity.subject, false, &["task-tracker:write"]);
    f.reader = actor(&scheduler_identity.subject, false, &["task-tracker:read"]);
    f.repo.config.reservation_scheduler_subject = scheduler.to_string();
    let foreign = crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let producer = crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let task = Box::pin(f.confirmed("native-scheduler", true)).await;
    let initial = f
        .repo
        .lifecycle_read(task, &f.reader)
        .await
        .unwrap()
        .receipt;
    // Activation validates every project, including those seeded by migrations.
    // Preserve existing bindings and add the missing synthetic owner receipts.
    let unbound =
        f.db.query_all(statement(
            "SELECT id FROM projects p WHERE NOT EXISTS
        (SELECT 1 FROM tracker_namespace_bindings b WHERE b.resource_id=p.id)",
            vec![],
        ))
        .await
        .unwrap();
    for row in unbound {
        let project: Uuid = row.try_get("", "id").unwrap();
        let registry = Uuid::new_v4();
        let namespace = Uuid::new_v4();
        let binding = json!({"schema_version":1,
            "namespace":{"registry_instance_id":registry,"namespace_id":namespace},
            "resource":{"kind":"tracker_project","instance_id":Uuid::new_v4(),"resource_id":project},
            "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
        exec(&f.db, "INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command)
            VALUES($1,$2,$3,1,'active',$4)",
            vec![project.into(),registry.into(),namespace.into(),binding.into()]).await.unwrap();
    }
    exec(
        &f.db,
        "INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state)
        SELECT registry_instance_id,namespace_id,1,state FROM tracker_namespace_bindings
        ON CONFLICT(registry_instance_id,space_id) DO NOTHING",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "UPDATE tracker_space_catalog SET admin_revision=1 WHERE admin_revision IS NULL",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id)
        SELECT resource_id,registry_instance_id,namespace_id FROM tracker_namespace_bindings
        ON CONFLICT(project_id,registry_instance_id,space_id) DO NOTHING",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "UPDATE tracker_space_admission_state SET active=true",
        vec![],
    )
    .await
    .unwrap();

    #[derive(Clone)]
    struct Authority {
        issuer: String,
        owner: Uuid,
        scheduler: Uuid,
        foreign: String,
        owner_permitted: bool,
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let authority = Arc::new(RwLock::new(Authority {
        issuer: issuer.clone(),
        owner,
        scheduler,
        foreign: foreign.subject.clone(),
        owner_permitted: true,
    }));
    let router = Router::new()
        .route("/oidc/jwks", get(|| async { HttpJson(json!({"keys":[]})) }))
        .route("/auth/tokens/introspect", get(
            |State(state): State<Arc<RwLock<Authority>>>, headers: HeaderMap| async move {
                let state = state.read().unwrap().clone();
                let (subject, scopes) = match headers["authorization"].to_str().unwrap() {
                    "Bearer sdlc_pat_scheduler_scope_write" => (state.scheduler.to_string(), vec!["task-tracker:write"]),
                    "Bearer sdlc_pat_scheduler_scope_read" => (state.scheduler.to_string(), vec!["task-tracker:read"]),
                    "Bearer sdlc_pat_scheduler_scope_mixed" => (state.scheduler.to_string(), vec!["task-tracker:read", "task-tracker:write"]),
                    "Bearer sdlc_pat_scheduler_scope_foreign" => (state.foreign, vec!["task-tracker:write"]),
                    _ => panic!("unexpected synthetic credential"),
                };
                let now = chrono::Utc::now();
                HttpJson(json!({"sub":subject,"email":"scheduler-scope@example.test","subject_role":"service_account",
                    "scopes":scopes,"observed_at":now,"expires_at":now+chrono::Duration::seconds(300)}))
            }))
        .route("/auth/access/decisions", post(
            |State(state): State<Arc<RwLock<Authority>>>, headers: HeaderMap, HttpJson(request): HttpJson<Json>| async move {
                assert_eq!(headers["authorization"], "Bearer scheduler-scope-owner-reader");
                let state = state.read().unwrap().clone();
                assert_eq!(request, json!({"schema_version":1,"subjects":[state.owner]}));
                let applications = if state.owner_permitted { vec!["task-tracker", "fleet-control"] } else { vec!["task-tracker"] };
                let now = chrono::Utc::now();
                HttpJson(json!({"schema_version":1,"decisions":[{"schema_version":1,"issuer":state.issuer,
                    "subject":state.owner,"role":"user","applications":applications,"revision":1,"enforced":true,
                    "observed_at":now,"expires_at":now+chrono::Duration::seconds(30)}]}))
            }))
        .with_state(authority.clone());
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/oidc/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED", "true");
        std::env::set_var(
            "TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN",
            "scheduler-scope-owner-reader",
        );
    }
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let bridge = &app::space_access::CENTRAL_BRIDGE;
    let write = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_write")
        .await
        .unwrap();
    let read = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_read")
        .await
        .unwrap();
    let mixed = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_mixed")
        .await
        .unwrap();
    let other = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_foreign")
        .await
        .unwrap();
    assert!(
        app::native_space_access::application_policy_required()
            .await
            .unwrap()
    );
    assert!(
        matches!(
            NativeRequest::verified(&write, true, false),
            Err(AppError::Forbidden)
        ),
        "the separate write-only credential cannot enter a GET request"
    );
    let scope = |proof| NativeRequest::verified(proof, true, true).unwrap();

    async fn denied(
        label: &str,
        f: &Fixture,
        task: Uuid,
        actor: &Principal,
        native: NativeRequest,
        command: LifecycleCommand,
    ) {
        let before = f.snapshot().await;
        let outcome = Box::pin(with_request(
            native,
            f.repo.lifecycle_command(task, actor, command),
        ))
        .await;
        assert!(
            matches!(&outcome, Err(AppError::Forbidden)),
            "{label}: expected Forbidden, got {outcome:?}"
        );
        assert_eq!(
            f.snapshot().await,
            before,
            "{label}: denial must not leave a command, hold or assignment"
        );
    }
    async fn scheduler_membership(f: &Fixture, present: bool) {
        // Synthetic owner admission for fixture administration, as in the
        // unified-space SQL tests; the normal owner/space/clock guards still run.
        let tx = f.db.begin().await.unwrap();
        let row = tx.query_one(statement("SELECT jsonb_build_object(
            'schema_version',1,'kind','human','user_id',u.id,
            'identity',jsonb_build_object('issuer','https://auth.example.test','subject',u.central_sub),
            'active_spaces',s.spaces,'readable_spaces',s.spaces,'policy_revision',1,'writable',true,
            'valid_until',floor(extract(epoch FROM clock_timestamp()))::bigint+30) AS actor
            FROM projects p JOIN users u ON u.id=p.owner_id AND u.is_active
            CROSS JOIN LATERAL (SELECT jsonb_agg(jsonb_build_object(
                'registry_instance_id',c.registry_instance_id,'space_id',c.space_id)) AS spaces
                FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id)
                WHERE l.project_id=p.id AND c.state='active' AND c.admin_revision IS NOT NULL) s
            WHERE p.id=$1", vec![f.project.into()])).await.unwrap().unwrap();
        let owner: Json = row.try_get("", "actor").unwrap();
        crate::guarded_db::bind_commit_clock(&tx, owner["valid_until"].as_i64().unwrap())
            .await
            .unwrap();
        exec(
            &tx,
            "SELECT set_config('pdlc.tracker.write_actor',$1,true)",
            vec![owner.to_string().into()],
        )
        .await
        .unwrap();
        let sql = if present {
            "INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub=$2"
        } else {
            "DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub=$2)"
        };
        exec(
            &tx,
            sql,
            vec![f.project.into(), f.writer.subject.clone().into()],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let mut next = LifecycleCommand::Reserve {
        expected_task_version: initial.task.version,
        machine_subject: Some(producer.subject.clone()),
        idempotency_key: "native-scheduler-reserve".into(),
    };
    for phase in ["reserve", "bind"] {
        // The same subject's legitimate GET frame cannot borrow its separate write credential.
        denied(
            &format!("{phase}/read_frame"),
            &f,
            task,
            &f.writer,
            NativeRequest::verified(&read, true, false).unwrap(),
            next.clone(),
        )
        .await;
        let mut unregistered = foreign.clone();
        unregistered.scopes = ["task-tracker:write".into()].into();
        denied(
            &format!("{phase}/unregistered_scheduler"),
            &f,
            task,
            &unregistered,
            scope(&other),
            next.clone(),
        )
        .await;
        let mut combined = f.writer.clone();
        combined.scopes.insert("task-tracker:read".into());
        denied(
            &format!("{phase}/mixed_scopes"),
            &f,
            task,
            &combined,
            scope(&mixed),
            next.clone(),
        )
        .await;

        scheduler_membership(&f, false).await;
        denied(
            &format!("{phase}/revoked_membership"),
            &f,
            task,
            &f.writer,
            scope(&write),
            next.clone(),
        )
        .await;
        scheduler_membership(&f, true).await;

        exec(
            &f.db,
            "UPDATE tracker_space_catalog SET state='restoring'",
            vec![],
        )
        .await
        .unwrap();
        denied(
            &format!("{phase}/restoring_space"),
            &f,
            task,
            &f.writer,
            scope(&write),
            next.clone(),
        )
        .await;
        exec(
            &f.db,
            "UPDATE tracker_space_catalog SET state='active'",
            vec![],
        )
        .await
        .unwrap();
        authority.write().unwrap().owner_permitted = false;
        denied(
            &format!("{phase}/revoked_owner_application"),
            &f,
            task,
            &f.writer,
            scope(&write),
            next.clone(),
        )
        .await;
        authority.write().unwrap().owner_permitted = true;

        let native = scope(&write);
        let original_deadline = native.deadline();
        let retained_clock = native.clone();
        let (saved, replay) = Box::pin(with_request(
            native,
            f.repo.lifecycle_command(task, &f.writer, next),
        ))
        .await
        .unwrap();
        assert!(!replay);
        assert!(
            retained_clock.deadline() <= original_deadline,
            "owner proof cannot renew the native clock"
        );
        if phase == "reserve" {
            assert_eq!(saved.task.runtime, RuntimeState::Reserved);
            assert_eq!(
                saved
                    .assignment
                    .as_ref()
                    .unwrap()
                    .machine_subject
                    .as_deref(),
                Some(producer.subject.as_str())
            );
            next = LifecycleCommand::BindRun {
                fence: fence(&saved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: producer.subject.clone(),
                    dispatch_receipt: ArtifactReceipt {
                        reference: "fixture://scheduler-native-acceptance".into(),
                        sha256: "a".repeat(64),
                    },
                },
                idempotency_key: "native-scheduler-bind".into(),
            };
        } else {
            assert_eq!(saved.task.runtime, RuntimeState::Running);
            assert_eq!(saved.binding.unwrap().machine_subject, producer.subject);
            break;
        }
    }
    let before_read = f.snapshot().await;
    let context = Box::pin(with_request(
        NativeRequest::verified(&read, true, false).unwrap(),
        f.repo.context(task, &f.reader),
    ))
    .await
    .unwrap();
    assert_eq!(context.task_id, task);
    assert_eq!(
        f.snapshot().await,
        before_read,
        "the separate reader keeps current context read-only"
    );
    server.abort();
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn automatic_queue_is_discovered_once_and_stage_start_replays_after_advancement() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = f.draft("automatic-advance", Some(DraftFlow::SingleTaskV1)).await;
        let document: RequirementsDocument = parse(json!({
            "goal":"Candidate fixture","scope":["scope"],"exclusions":[],
            "scenarios":["scenario"],"acceptance_criteria":["criterion"],"constraints":[],
            "dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["pre"]}));
        let (reservation, _) = crate::sdlc::configured_test_support::confirmed_pm(
            &f.repo, task, &f.owner, f.roles.project_manager.agent_id, document, false,
        ).await;
        Box::pin(f.confirmed_with_rollout("not-started", false, true)).await;
        assert!(f.repo.read_delivery_candidates(&f.reader, f.query(None, 100)).await.unwrap().items.is_empty());
        let mut state = f.repo.read(task, &f.owner).await.unwrap();
        let assignment = state.assignment.as_ref().unwrap();
        let pm = actor(&assignment.machine_subject, false, &["task-tracker:read", "task-tracker:write"]);
        let completed = crate::sdlc::configured_test_support::complete_pm(&f.repo, task, &f.owner, &pm, &reservation).await;
        assert_eq!(completed.stage, Stage::Draft);
        assert_eq!(completed.status, LifecycleStatus::Completed);
        let initial = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        assert_eq!(initial.task.stage, LifecycleStage::Analysis);
        assert_eq!(initial.task.status, LifecycleStatus::Ready);
        assert_eq!(initial.task.runtime, RuntimeState::Queued);
        assert_eq!(initial.task.version, 1);
        let intent = f.repo.analysis_intent(task, &f.owner).await.unwrap();
        assert_eq!(intent.confirmation_id, initial.task.routing.confirmation_id);
        assert_eq!(intent.workflow, initial.task.routing.policy.routes.analyst.workflow_key);
        // The pure tuple validator uses the PM assignment retained at the handoff boundary.
        state.stage = Stage::Analysis;
        state.status = Some(LifecycleStatus::Ready);
        app::sdlc_lifecycle::configured_started_intent(&state, &completed, &initial, intent.intent_id).unwrap();
        assert!(app::sdlc::analysis_intent(&state, state.current_confirmation().unwrap(), intent.intent_id).is_err());
        for field in ["task_id", "owner_subject", "version", "stage", "runtime"] {
            let mut raw = json!(initial);
            raw["task"][field] = match field {
                "task_id" => json!(Uuid::new_v4()),
                "version" => json!(2),
                "stage" => json!("Backlog"),
                "runtime" => json!("running"),
                _ => json!("foreign"),
            };
            let wrong: LifecycleReceipt = parse(raw);
            assert!(app::sdlc_lifecycle::configured_started_intent(&state, &completed, &wrong, intent.intent_id).is_err(), "{field}");
        }
        let before_discovery = f.snapshot().await;
        let original_command = command(&f, task, "manual-after-auto").await;
        assert!(matches!(f.repo.lifecycle_command(task, &f.owner, original_command.clone()).await, Err(AppError::Conflict(_))));
        assert!(matches!(f.repo.lifecycle_operation(task, &f.owner, "manual-after-auto").await, Err(AppError::NotFound(_))));
        assert_eq!(f.snapshot().await, before_discovery);
        let page = f.repo.read_delivery_candidates(&f.reader, f.query(None, 1)).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].task_id, task);
        assert_eq!(f.snapshot().await, before_discovery);
        assert_eq!(page.items[0].next_operation, domain::sdlc_candidates::CandidateOperation::Reserve);
        let counts = f.db.query_one(statement("SELECT
            (SELECT count(*) FROM sdlc_analysis_intents WHERE task_id=$1) AS intents,
            (SELECT count(*) FROM sdlc_delivery_operations WHERE task_id=$1 AND command IS NOT NULL) AS starts,
            (SELECT count(*) FROM sdlc_outbox WHERE task_id=$1 AND event_type IN ('analysis.intent_created','delivery.queued')) AS events,
            (SELECT count(*) FROM sdlc_delivery_projection_permits WHERE task_id=$1) AS permits",
            vec![task.into()])).await.unwrap().unwrap();
        assert_eq!(counts.try_get::<i64>("", "intents").unwrap(), 1);
        assert_eq!(counts.try_get::<i64>("", "starts").unwrap(), 1);
        assert_eq!(counts.try_get::<i64>("", "events").unwrap(), 2);
        assert_eq!(counts.try_get::<i64>("", "permits").unwrap(), 0);
        // Existing configured role-result path advances the same task without another human action.
        let producer_user = Uuid::new_v4();
        exec(&f.db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
            VALUES($1,'start-agent@example.test','start-agent','Start Agent','!','77777777-7777-4777-8777-777777777777',true)",
            vec![producer_user.into()]).await.unwrap();
        exec(&f.db, "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
            vec![f.project.into(), producer_user.into()]).await.unwrap();
        let storage = domain::InMemoryStorage::default();
        let producer = actor("77777777-7777-4777-8777-777777777777", false, &["task-tracker:read", "task-tracker:write"]);
        // Bind only: the shared bind fixture also starts the stage, which this case tests explicitly.
        let reserved = f.repo.lifecycle_command(task, &f.writer, LifecycleCommand::Reserve {
            expected_task_version: initial.task.version,
            machine_subject: Some(producer.subject.clone()),
            idempotency_key: format!("reserve-{}", initial.task.version),
        }).await.unwrap().0;
        let bound = f.repo.lifecycle_command(task, &f.writer, LifecycleCommand::BindRun {
            fence: fence(&reserved),
            binding: RunBinding {
                chat_id: Uuid::new_v4(),
                run_id: Uuid::new_v4(),
                machine_subject: producer.subject.clone(),
                dispatch_receipt: ArtifactReceipt {
                    reference: "fixture://synthetic-dispatch".into(),
                    sha256: "a".repeat(64),
                },
            },
            idempotency_key: format!("bind-{}", reserved.task.version),
        }).await.unwrap().0;
        assert_eq!(bound.task.status, LifecycleStatus::Ready);
        assert_eq!(bound.task.runtime, RuntimeState::Running);
        let start = domain::sdlc_stage_status::StartStage {
            fence: domain::sdlc_stage_status::StageStartFence::Delivery(fence(&bound)),
            idempotency_key: "analysis-stage-start".into(),
        };
        let (started, replay) = f.repo.start_stage(task, &producer, start.clone()).await.unwrap();
        assert!(!replay);
        assert_eq!(started.status, LifecycleStatus::InProgress);
        let running_head = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        assert_eq!(running_head.task.status, LifecycleStatus::InProgress);
        let running = Box::pin(output(&f, task, running_head, &storage)).await;
        let terminal = Box::pin(proposal(&f, &running)).await;
        let advanced = f.repo.lifecycle_command(task, &f.writer, LifecycleCommand::Complete {
            fence: fence(&running.head), terminal, result: result(&running), idempotency_key: "accepted-analysis".into(),
        }).await.unwrap().0;
        assert_eq!(advanced.task.task_id, task);
        assert_eq!(advanced.task.stage, LifecycleStage::Architecture);
        assert_eq!(advanced.task.status, LifecycleStatus::Ready);
        assert_eq!(advanced.task.runtime, RuntimeState::Queued);
        let before = f.snapshot().await;
        assert_eq!(json!(f.repo.stage_start_operation(task, &producer, &start.idempotency_key).await.unwrap()), json!(started));
        let (saved, replay) = f.repo.start_stage(task, &producer, start).await.unwrap();
        assert!(replay);
        assert_eq!(json!(saved), json!(started));
        assert!(matches!(f.repo.lifecycle_command(task, &f.owner, original_command).await, Err(AppError::Conflict(_))));
        assert!(matches!(f.repo.lifecycle_operation(task, &f.owner, "manual-after-auto").await, Err(AppError::NotFound(_))));
        assert_eq!(f.snapshot().await, before);
        let current = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        assert_eq!(json!(current), json!(advanced));
        let page = f.repo.read_delivery_candidates(&f.reader, f.query(None, 100)).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].task_id, task);
        assert_eq!(f.snapshot().await, before);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn start_refuses_nonowners_no_opt_in_missing_consent_stale_and_foreign_receipts() {
    Box::pin(async {
        let mut f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("guarded", true)).await;
        let cmd = command(&f, task, "start").await;
        let mut state = f.repo.pm_completion_state(task, &f.owner).await.unwrap();
        let completed = f
            .repo
            .configured_pm_completion(task, &f.owner, None)
            .await
            .unwrap()
            .unwrap();
        let completed = completed.result;
        // Pure admission-tuple cases use the exact saved PM tuple at the pre-handoff boundary.
        // No database row or historical receipt is written by this reconstruction.
        state.stage = Stage::Draft;
        state.status = Some(LifecycleStatus::Completed);
        state.retained_fields.remove("lifecycle");
        app::sdlc_lifecycle::validate_start(&state, &completed).unwrap();
        for (field, value) in [
            ("owner_subject", json!("foreign")),
            ("project_id", json!(Uuid::new_v4())),
            ("confirmation_revision", json!(2)),
            ("confirmations", json!([])),
        ] {
            let mut raw = json!(state);
            raw[field] = value;
            let stale: TaskState = parse(raw);
            assert!(
                app::sdlc_lifecycle::validate_start(&stale, &completed).is_err(),
                "{field}"
            );
        }
        let mut stale = state.clone();
        stale.revisions.last_mut().unwrap().content_hash = "d".repeat(64);
        assert!(app::sdlc_lifecycle::validate_start(&stale, &completed).is_err());
        let mut stale = state.clone();
        stale.assignment.as_mut().unwrap().version += 1;
        assert!(app::sdlc_lifecycle::validate_start(&stale, &completed).is_err());
        let before = f.snapshot().await;
        for principal in [
            f.writer.clone(),
            actor("owner", false, &[]),
            actor("foreign", true, &["admin"]),
            actor("scheduler", true, &["admin"]),
        ] {
            assert!(matches!(
                f.repo
                    .lifecycle_command(task, &principal, cmd.clone())
                    .await,
                Err(AppError::Forbidden)
            ));
        }
        f.repo.config.single_task_project_ids.clear();
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, cmd.clone()).await,
            Err(AppError::Unavailable(_))
        ));
        f.repo.config.single_task_project_ids.insert(f.project);
        let wrong = LifecycleCommand::StartAnalysis {
            pm_completion_id: Uuid::new_v4(),
            idempotency_key: "wrong-receipt".into(),
        };
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, wrong).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(f.snapshot().await, before);
        let unconfirmed = f.draft("no-consent", Some(DraftFlow::SingleTaskV1)).await;
        let original_project = f.project;
        f.project = Uuid::new_v4();
        f.repo.config.single_task_project_ids.insert(f.project);
        exec(&f.db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) SELECT $1,'OTHER','Other synthetic project',owner_id,$2 FROM projects WHERE id=$3",
            vec![f.project.into(),Uuid::new_v4().into(),original_project.into()]).await.unwrap();
        exec(&f.db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Other','[]' FROM projects WHERE id=$1",vec![f.project.into()]).await.unwrap();
        crate::sdlc::source::configure_test_source(&f.repo, f.project, &f.owner).await;
        f.repo.set_routing_policy(f.project,&f.owner,SetRoutingPolicy {
            expected_version:None,routes:f.roles.clone(),idempotency_key:"other-policy".into(),
        }).await.unwrap();
        let foreign = Box::pin(f.confirmed("another-project-receipt", true)).await;
        let foreign_command = command(&f, foreign, "foreign-receipt").await;
        f.project = original_project;
        let before = f.snapshot().await;
        for target in [task, unconfirmed] {
            assert!(matches!(
                f.repo
                    .lifecycle_command(target, &f.owner, foreign_command.clone())
                    .await,
                Err(AppError::Conflict(_))
            ));
        }
        assert_eq!(f.snapshot().await, before);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn manual_start_races_after_autoqueue_and_current_access_do_not_duplicate_effects() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("race", true)).await;
        let cmd = command(&f, task, "race-key").await;
        let before = f.snapshot().await;
        let (a, b) = tokio::join!(
            f.repo.lifecycle_command(task, &f.owner, cmd.clone()),
            f.repo.lifecycle_command(task, &f.owner, cmd.clone())
        );
        assert!(matches!(a, Err(AppError::Conflict(_))));
        assert!(matches!(b, Err(AppError::Conflict(_))));
        assert_eq!(f.snapshot().await, before);
        let other = Box::pin(f.confirmed("different-key-race", true)).await;
        let before = f.snapshot().await;
        let first = command(&f, other, "first").await;
        let second = command(&f, other, "second").await;
        let (first, second) = tokio::join!(
            f.repo.lifecycle_command(other, &f.owner, first),
            f.repo.lifecycle_command(other, &f.owner, second)
        );
        assert!(matches!(first, Err(AppError::Conflict(_))));
        assert!(matches!(second, Err(AppError::Conflict(_))));
        assert_eq!(f.snapshot().await, before);
        assert!(matches!(f.repo.lifecycle_operation(task, &f.owner, "race-key").await, Err(AppError::NotFound(_))));
        let before = f.snapshot().await;
        let changed = LifecycleCommand::StartAnalysis {
            pm_completion_id: Uuid::new_v4(),
            idempotency_key: "race-key".into(),
        };
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, changed).await,
            Err(AppError::Conflict(_))
        ));
        let new_key = command(&f, task, "second-key").await;
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, new_key).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(f.snapshot().await, before);
        exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='foreign') WHERE id=$1",vec![f.project.into()]).await.unwrap();
        assert!(matches!(f.repo.lifecycle_operation(task,&f.owner,"race-key").await,Err(AppError::Forbidden)));
        assert!(matches!(f.repo.lifecycle_command(task,&f.owner,cmd.clone()).await,Err(AppError::Forbidden)));
        exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='owner') WHERE id=$1",vec![f.project.into()]).await.unwrap();
        exec(
            &f.db,
            "UPDATE users SET is_active=false WHERE central_sub='owner'",
            vec![],
        )
        .await
        .unwrap();
        assert!(matches!(
            f.repo.lifecycle_operation(task, &f.owner, "race-key").await,
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, cmd).await,
            Err(AppError::Forbidden)
        ));
        assert_eq!(f.snapshot().await, before);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn start_sql_rejects_detached_command_and_direct_permit_bypass() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("sql-guard", true)).await;
        let receipt = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        let detached = Box::pin(f.confirmed("detached", true)).await;
        let detached_cmd = command(&f, detached, "detached-key").await;
        let mut forged = receipt.clone();
        forged.task.task_id = detached;
        let state = f.repo.read(detached, &f.owner).await.unwrap();
        let before = f.snapshot().await;
        let tx = f.db.begin().await.unwrap();
        assert!(PostgresSdlcRepository::save_delivery_projection(&tx, &f.owner, &forged, true).await.is_err());
        tx.rollback().await.unwrap();
        // A detached owner operation cannot enter an already-published queue.
        let row = f.db.query_one(statement("SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1", vec![detached.into()])).await.unwrap().unwrap();
        forged.task.routing = parse(row.try_get("", "payload").unwrap());
        let tx = f.db.begin().await.unwrap();
        let error = tx.execute(statement("INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id,command,transaction_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,pg_current_xact_id())", vec![detached.into(), f.owner.subject.clone().into(), detached_cmd.key().into(),
            app::sdlc::canonical_hash(&detached_cmd).unwrap().into(), json_value(&forged).unwrap(), Uuid::new_v4().into(), json_value(&detached_cmd).unwrap()])).await.unwrap_err();
        assert!(error.to_string().contains("SDLC explicit start requires current owner/completion/consent/source"), "{error}");
        tx.rollback().await.unwrap();
        assert_eq!(f.snapshot().await, before);
        assert_eq!(state.stage, Stage::Analysis);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn pm_completion_automatically_queues_once_without_impersonating_owner() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed_with_rollout("automatic", true, true)).await;
        let state = f.repo.read(task, &f.owner).await.unwrap();
        assert_eq!(state.stage, Stage::Analysis);
        assert_eq!(state.status, Some(LifecycleStatus::Ready));
        let receipt = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        assert_eq!(receipt.task.runtime, RuntimeState::Queued);
        assert!(receipt.assignment.is_none());
        let completion = f.repo.configured_pm_completion(task, &f.owner, None).await.unwrap().unwrap();
        assert_eq!(completion.result.stage, Stage::Draft); // Immutable PM fact, not the current cursor.
        assert_eq!(completion.result.status, LifecycleStatus::Completed);
        assert!(f.repo.context(task, &f.owner).await.unwrap().assignment.is_none());
        let intent = f.repo.analysis_intent(task, &f.owner).await.unwrap();
        assert_eq!(intent.content_hash, completion.result.content_hash);
        assert_eq!(intent.confirmation_id, completion.result.confirmation_id);
        let row = f.db.query_one(statement("SELECT actor_subject,transaction_id=(SELECT transaction_id FROM sdlc_configured_pm_completions WHERE completion_id=$2) AS same_transaction FROM sdlc_delivery_operations WHERE task_id=$1 AND command IS NOT NULL", vec![task.into(),completion.result.completion_id.into()])).await.unwrap().unwrap();
        assert_ne!(row.try_get::<String>("", "actor_subject").unwrap(), f.owner.subject);
        let pm_subject = row.try_get::<String>("", "actor_subject").unwrap();
        assert_eq!(pm_subject, state.assignment.as_ref().unwrap().machine_subject);
        let pm = actor(&pm_subject, false, &["task-tracker:read", "task-tracker:write"]);
        assert!(f.repo.read(task, &pm).await.is_err());
        let saved = f.repo.configured_pm_completion(task, &pm, None).await.unwrap().unwrap();
        assert_eq!(saved.result.completion_id, completion.result.completion_id);
        assert!(f.repo.pm_completion_state(task, &pm).await.is_ok());
        let foreign = actor(&Uuid::new_v4().to_string(), false, &["task-tracker:read", "task-tracker:write"]);
        assert!(f.repo.configured_pm_completion(task, &foreign, None).await.is_err());
        assert!(row.try_get::<bool>("", "same_transaction").unwrap());
        let before = f.snapshot().await;
        for _ in 0..2 {
            let current = f.repo.read_delivery_candidates(&f.reader, f.query(None,100)).await.unwrap();
            assert_eq!(current.items.len(),1);
            assert_eq!(current.items[0].task_id,task);
        }
        assert_eq!(before,f.snapshot().await);
        assert!(matches!(f.repo.lifecycle_command(task,&f.owner,command(&f,task,"manual-after-auto").await).await,Err(AppError::Conflict(_))));
        assert_eq!(before,f.snapshot().await);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn automatic_start_requires_completion_and_refuses_unpermitted_projection() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let pending = Box::pin(f.confirmed_with_rollout("consent-only",false,true)).await;
        assert_eq!(f.repo.read(pending,&f.owner).await.unwrap().stage,Stage::Draft);
        assert!(f.repo.analysis_intent(pending,&f.owner).await.is_err());
        let before=f.snapshot().await;
        let tx=f.db.begin().await.unwrap();
        assert!(tx.execute(statement("UPDATE sdlc_tasks SET state=state||'{\"stage\":\"Analysis\",\"status\":\"Ready\"}'::jsonb WHERE task_id=$1",vec![pending.into()])).await.is_err());
        tx.rollback().await.unwrap();
        assert_eq!(before,f.snapshot().await);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn forward_auto_start_preserves_completed_backlog_and_manual_start_history() {
    Box::pin(async {
        let migrations=migration::Migrator::migrations();
        let checkpoint=migrations.iter().position(|migration| migration.name() == PRE_AUTO_START).unwrap()+1;
        let f=Box::pin(Fixture::new_with_migration_steps(Some(checkpoint as u32))).await;
        let (retained, retained_completion)=Box::pin(predecessor_pm(&f,"pre-auto",PRE_AUTO_START)).await;
        let (started, completed)=Box::pin(predecessor_pm(&f,"manual-start",PRE_AUTO_START)).await;
        let guards = predecessor_boundary(&f, PRE_AUTO_START).await;
        let tx=f.db.begin().await.unwrap();
        let state=f.repo.load(&tx,started,&f.owner).await.unwrap();
        assert_eq!(state.stage, Stage::Backlog);
        assert_eq!(state.status, Some(LifecycleStatus::Ready));
        assert!(state.owner(&f.owner) && !state.retained_fields.contains_key("lifecycle"));
        let consent=state.current_confirmation().unwrap();
        assert_eq!(consent.id, completed.confirmation_id);
        let row=tx.query_one(statement("SELECT s.payload FROM sdlc_task_routing_snapshots s
            JOIN sdlc_task_source_bindings b ON b.task_id=s.task_id AND b.confirmation_id=s.confirmation_id
                AND b.routing_snapshot_id=s.snapshot_id AND b.requirement_revision=s.requirement_revision AND b.content_hash=s.content_hash
            WHERE s.task_id=$1 AND s.confirmation_id=$2",vec![started.into(),consent.id.into()])).await.unwrap().unwrap();
        let routing: domain::sdlc_routing::TaskRoutingSnapshot=parse(row.try_get("","payload").unwrap());
        let original_command=LifecycleCommand::StartAnalysis {pm_completion_id:completed.completion_id,idempotency_key:"historical:owner-start".into()};
        let original: LifecycleReceipt=parse(json!({"task":{"task_id":started,"owner_subject":state.owner_subject,
            "version":1,"stage":"Analysis","status":"Ready","cycle":0,"transient_retries":0,
            "waiting_reason":"queued_for_assignment","runtime":"queued","routing":routing,"previous_result_id":null},
            "assignment":null,"lease":null,"binding":null,"result":null}));
        let intent: AnalysisIntent=parse(json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":started,"root_task_id":started,"intent_id":Uuid::new_v4(),
            "confirmation_id":consent.id,"requirement_revision":consent.revision,"content_hash":consent.content_hash,
            "stage":"Analysis","status":"Ready","role":"Analyst","workflow":routing.policy.routes.analyst.workflow_key,
            "mode":"analysis","scope":"business","cycle":0,"attempt":0,
            "operation_key":format!("analysis:{}",consent.id),"created_at":consent.created_at}));
        let event=Uuid::new_v4();
        // Original111 sequence: command journal, intent, exact projection permits/head, outbox, COMMIT.
        // The original INSERT and deferred commit triggers validate this complete synthetic tuple.
        exec(&tx,"INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id,command,transaction_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,pg_current_xact_id())",vec![started.into(),f.owner.subject.clone().into(),original_command.key().into(),
                app::sdlc::canonical_hash(&original_command).unwrap().into(),json_value(&original).unwrap(),event.into(),json_value(&original_command).unwrap()]).await.unwrap();
        exec(&tx,"INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,routing_snapshot_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8)",vec![intent.intent_id.into(),started.into(),intent.requirement_revision.into(),intent.content_hash.clone().into(),
                consent.id.into(),intent.operation_key.clone().into(),json_value(&intent).unwrap(),routing.snapshot_id.into()]).await.unwrap();
        PostgresSdlcRepository::save_delivery_projection(&tx,&f.owner,&original,true).await.unwrap();
        for (id,kind,result) in [(intent.intent_id,"analysis.intent_created",json!(intent)),(event,"delivery.queued",json!(original))] {
            exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",vec![id.into(),started.into(),kind.into(),
                json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,
                    "task_id":started,"root_task_id":started,"owner_subject":state.owner_subject,"stage":"Analysis",
                    "requirement_revision":consent.revision,"result":result}).into()]).await.unwrap();
        }
        tx.commit().await.unwrap();
        assert_eq!(predecessor_boundary(&f, PRE_AUTO_START).await, guards);
        // Historical images can have LF function bodies while the new source uses CRLF.
        for function in ["sdlc_critical_pm_lifecycle_insert_gate()", "sdlc_critical_pm_lifecycle_commit_gate()", "sdlc_configured_cli_completion_transition()"] {
            let row=f.db.query_one(statement("SELECT replace(pg_get_functiondef($1::regprocedure),chr(13)||chr(10),chr(10)) AS definition",vec![function.into()])).await.unwrap().unwrap();
            f.db.execute_unprepared(&row.try_get::<String>("","definition").unwrap()).await.unwrap();
        }
        let before=f.snapshot().await;
        let names=protected_tables(&f).await;
        let history=protected_history(&f,&names).await;
        assert_eq!(history["sdlc_configured_pm_completions"].as_array().unwrap().len(),2);
        assert_eq!(history["sdlc_delivery_operations"].as_array().unwrap().len(),1);
        let mut completion_tasks: Vec<_> = history["sdlc_configured_pm_completions"].as_array().unwrap().iter()
            .map(|row| row["task_id"].as_str().unwrap().to_owned()).collect();
        completion_tasks.sort();
        let mut expected_tasks=vec![retained.to_string(),started.to_string()];
        expected_tasks.sort();
        assert_eq!(completion_tasks,expected_tasks);
        assert_eq!(history["sdlc_delivery_operations"][0]["command"],json!(original_command));
        assert_eq!(history["sdlc_delivery_operations"][0]["result"],json!(original));
        upgrade_preserving_predecessor(&f,&names,history).await;
        assert_eq!(before,f.snapshot().await);
        f.assert_backlog_without_analysis(retained).await;
        let preserved=protected_history(&f,&names).await;
        assert_eq!(json!(f.repo.configured_pm_completion(retained,&f.owner,None).await.unwrap().unwrap().result),json!(retained_completion));
        assert_eq!(json!(f.repo.configured_pm_completion(started,&f.owner,None).await.unwrap().unwrap().result),json!(completed));
        assert_eq!(json!(f.repo.lifecycle_operation(started,&f.owner,original_command.key()).await.unwrap()),json!(original));
        let (replayed,replay)=f.repo.lifecycle_command(started,&f.owner,original_command.clone()).await.unwrap();
        assert!(replay);
        assert_eq!(json!(replayed),json!(original));
        let changed=LifecycleCommand::StartAnalysis {pm_completion_id:Uuid::new_v4(),idempotency_key:original_command.key().into()};
        assert!(matches!(f.repo.lifecycle_command(started,&f.owner,changed).await,Err(AppError::Conflict(_))));
        for (task,completion) in [(started,&completed),(retained,&retained_completion)] {
            let new=LifecycleCommand::StartAnalysis {pm_completion_id:completion.completion_id,idempotency_key:"new-old-start".into()};
            assert!(matches!(f.repo.lifecycle_command(task,&f.owner,new).await,Err(AppError::Conflict(_))));
            assert!(matches!(f.repo.lifecycle_operation(task,&f.owner,"new-old-start").await,Err(AppError::NotFound(_))));
        }
        for caller in [&f.writer,&actor("foreign",true,&["admin"])] {
            assert!(matches!(f.repo.lifecycle_operation(started,caller,original_command.key()).await,Err(AppError::Forbidden)));
            assert!(matches!(f.repo.lifecycle_command(started,caller,original_command.clone()).await,Err(AppError::Forbidden)));
        }
        assert_eq!(protected_history(&f,&names).await,preserved);
        exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='foreign') WHERE id=$1",vec![f.project.into()]).await.unwrap();
        assert!(matches!(f.repo.lifecycle_operation(started,&f.owner,original_command.key()).await,Err(AppError::Forbidden)));
        assert!(matches!(f.repo.lifecycle_command(started,&f.owner,original_command.clone()).await,Err(AppError::Forbidden)));
        exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='owner') WHERE id=$1",vec![f.project.into()]).await.unwrap();
        exec(&f.db,"UPDATE users SET is_active=false WHERE central_sub='owner'",vec![]).await.unwrap();
        assert!(matches!(f.repo.lifecycle_operation(started,&f.owner,original_command.key()).await,Err(AppError::Forbidden)));
        assert!(matches!(f.repo.lifecycle_command(started,&f.owner,original_command).await,Err(AppError::Forbidden)));
        exec(&f.db,"UPDATE users SET is_active=true WHERE central_sub='owner'",vec![]).await.unwrap();
        let page=f.repo.read_delivery_candidates(&f.reader,f.query(None,100)).await.unwrap();
        assert!(page.items.iter().all(|candidate|candidate.task_id!=retained));
        assert_eq!(protected_history(&f,&names).await,preserved);
        assert_eq!(before,f.snapshot().await);
    }).await;
}
