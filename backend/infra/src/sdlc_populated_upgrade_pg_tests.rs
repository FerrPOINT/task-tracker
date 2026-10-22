//! Child of draft_flow_tests; main owns registration and disposable PostgreSQL execution.
//! All business data is synthetic. Chat references are metadata, not Fleet chat storage.
use super::*;
use std::collections::BTreeMap;

async fn scalar_json(db: &impl ConnectionTrait, sql: &str, values: Vec<Value>) -> Json {
    db.query_one(statement(sql, values))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap()
}

async fn ledger(db: &DatabaseConnection, expected: &[String]) -> Json {
    let rows = db
        .query_all(statement(
            "SELECT version FROM seaql_migrations ORDER BY version",
            vec![],
        ))
        .await
        .unwrap();
    let actual: Vec<String> = rows
        .iter()
        .map(|r| r.try_get("", "version").unwrap())
        .collect();
    let mut ordered = expected.to_vec();
    ordered.sort();
    assert_eq!(actual, ordered);
    scalar_json(db, "SELECT COALESCE(jsonb_agg(to_jsonb(m) ORDER BY version),'[]') AS value FROM seaql_migrations m", vec![]).await
}

async fn tables(db: &DatabaseConnection) -> Vec<String> {
    db.query_all(statement(
        r"SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename LIKE 'sdlc\_%' ESCAPE '\' ORDER BY tablename",
        vec![],
    )).await.unwrap().iter().map(|r| r.try_get("", "tablename").unwrap()).collect()
}

async fn history(
    db: &DatabaseConnection,
    names: &[String],
    upgraded: bool,
) -> BTreeMap<String, Json> {
    let mut snapshot = BTreeMap::new();
    for name in names {
        // Identifiers come only from pg_tables, never a caller or stored business payload.
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        let row = if upgraded && name == "sdlc_analysis_intents" {
            "to_jsonb(t)-'routing_generation'-'review_routing_snapshot_id'"
        } else if name == "sdlc_pm_lease_operations" {
            "to_jsonb(t)-'recovery'"
        } else {
            "to_jsonb(t)"
        };
        let sql = format!(
            "SELECT COALESCE(jsonb_agg(value ORDER BY value::text),'[]') AS value FROM (SELECT {row} AS value FROM {name} t) s"
        );
        snapshot.insert(name.clone(), scalar_json(db, &sql, vec![]).await);
    }
    snapshot
}

struct OriginalData {
    project: Uuid,
    owner: Uuid,
    issues: [Uuid; 2],
    status: Uuid,
    requirements_ref: Uuid,
    chat_ref: Uuid,
}

impl OriginalData {
    async fn seed(db: &DatabaseConnection) -> Self {
        let original = Self {
            project: Uuid::new_v4(),
            owner: Uuid::new_v4(),
            issues: [Uuid::new_v4(), Uuid::new_v4()],
            status: Uuid::new_v4(),
            requirements_ref: Uuid::new_v4(),
            chat_ref: Uuid::new_v4(),
        };
        for (subject, id) in [
            ("owner", original.owner),
            ("pm", Uuid::new_v4()),
            ("verifier", Uuid::new_v4()),
        ] {
            exec(db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
                vec![id.into(),format!("upgrade-{subject}@example.test").into(),subject.into()]).await.unwrap();
        }
        exec(db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'UPG','Preserved populated project',$2,$3)",
            vec![original.project.into(),original.owner.into(),Uuid::new_v4().into()]).await.unwrap();
        exec(db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Preserved board','[]' FROM projects WHERE id=$1",
            vec![original.project.into()]).await.unwrap();
        exec(db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub IN ('pm','verifier')",
            vec![original.project.into()]).await.unwrap();
        exec(db,"INSERT INTO statuses(id,name,category,position,is_default,is_closed) VALUES($1,'Upgrade preserved','todo',17,false,false)",
            vec![original.status.into()]).await.unwrap();
        let metadata = json!({"requirements_ref":original.requirements_ref,"chat_ref":original.chat_ref,
            "source":"synthetic metadata preserved in Tracker; no chat storage"});
        let field = Uuid::new_v4();
        exec(db,"INSERT INTO custom_fields(id,project_id,name,field_type,options,is_required) VALUES($1,$2,'Preserved references','text','[]',false)",
            vec![field.into(),original.project.into()]).await.unwrap();
        for (id, key, deleted) in [
            (original.issues[0], "UPG-7", false),
            (original.issues[1], "UPG-41", true),
        ] {
            exec(db,r#"INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,description,reporter_id,assignee_id,priority,labels,position,time_spent_seconds,deleted_at)
                 VALUES($1,$2,$3,'task',$4,'Preserved requirement discussion',$5,$6,$6,'medium','["preserved"]',2.5,17,CASE WHEN $7 THEN TIMESTAMPTZ '2026-10-01 00:00:00+00' ELSE NULL END)"#,
                vec![id.into(),original.project.into(),key.into(),original.status.into(),
                    format!("Original requirements {} and chat metadata {}",original.requirements_ref,original.chat_ref).into(),original.owner.into(),deleted.into()]).await.unwrap();
            exec(
                db,
                "INSERT INTO comments(id,issue_id,author_id,body) VALUES($1,$2,$3,$4)",
                vec![
                    Uuid::new_v4().into(),
                    id.into(),
                    original.owner.into(),
                    metadata.to_string().into(),
                ],
            )
            .await
            .unwrap();
            exec(
                db,
                "INSERT INTO issue_custom_field_values(issue_id,field_id,value) VALUES($1,$2,$3)",
                vec![id.into(), field.into(), metadata.clone().into()],
            )
            .await
            .unwrap();
            exec(db,"INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id) VALUES($1,$2,NULL,$3,$4)",
                vec![Uuid::new_v4().into(),id.into(),original.status.into(),original.owner.into()]).await.unwrap();
        }
        original
    }

    async fn snapshot(&self, db: &DatabaseConnection, namespaced: bool) -> Json {
        let project_row = if namespaced {
            "to_jsonb(p)-'namespace_managed'"
        } else {
            "to_jsonb(p)"
        };
        scalar_json(db, &format!(
            "SELECT jsonb_build_object(
             'project',{project_row},
             'users',(SELECT jsonb_agg(to_jsonb(u) ORDER BY id) FROM users u WHERE central_sub IN ('owner','pm','verifier')),
             'board',(SELECT to_jsonb(b) FROM boards b WHERE b.id=p.default_board_id),
             'members',(SELECT jsonb_agg(to_jsonb(m) ORDER BY user_id) FROM project_members m WHERE project_id=p.id),
             'issues',(SELECT jsonb_agg(to_jsonb(i) ORDER BY id) FROM issues i WHERE id IN ($2,$3)),
             'comments',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM comments c WHERE issue_id IN ($2,$3)),
             'custom_fields',(SELECT jsonb_agg(to_jsonb(f) ORDER BY id) FROM custom_fields f WHERE project_id=p.id),
             'custom_values',(SELECT jsonb_agg(to_jsonb(v) ORDER BY issue_id,field_id) FROM issue_custom_field_values v WHERE issue_id IN ($2,$3)),
             'status',(SELECT to_jsonb(s) FROM statuses s WHERE id=$4),
             'status_history',(SELECT jsonb_agg(to_jsonb(h) ORDER BY id) FROM issue_status_history h WHERE issue_id IN ($2,$3))) AS value
             FROM projects p WHERE p.id=$1"
        ),vec![self.project.into(),self.issues[0].into(),self.issues[1].into(),self.status.into()]).await
    }
}

struct OriginalCommand {
    actor: Principal,
    command: SdlcCommand,
    body: Json,
    sha256: String,
    result: Json,
}

// Historical095 confirmation queued Analysis. This fixture explicitly reconstructs
// that predecessor policy; the current PM producer stops at Backlog instead.
async fn finish_predecessor_confirmation(
    tx: &DatabaseTransaction,
    state: &TaskState,
    original: &OriginalCommand,
) {
    assert!(matches!(original.command, SdlcCommand::Confirm { .. }));
    assert_eq!(state.stage, Stage::Analysis);
    assert!(state.status.is_none());
    assert!(state.draft_flow().unwrap().is_none());
    let task = state.task_id;
    let confirmation = state.confirmations.last().unwrap();
    let row = tx.query_one(statement(
        "SELECT i.payload,s.payload AS routing_payload FROM sdlc_analysis_intents i
         JOIN sdlc_task_routing_snapshots s ON s.snapshot_id=i.routing_snapshot_id
         WHERE i.task_id=$1 AND i.confirmation_id=$2 AND i.requirement_revision=$3 AND i.content_hash=$4",
        vec![task.into(),confirmation.id.into(),confirmation.revision.into(),confirmation.content_hash.clone().into()],
    )).await.unwrap().unwrap();
    let intent: AnalysisIntent = parse(row.try_get("", "payload").unwrap());
    let routing: domain::sdlc_routing::TaskRoutingSnapshot =
        parse(row.try_get("", "routing_payload").unwrap());
    assert_eq!(routing.contract_version, 1);
    assert_eq!(routing.task_id, task);
    assert_eq!(routing.confirmation_id, confirmation.id);
    assert_eq!(routing.requirement_revision, confirmation.revision);
    assert_eq!(routing.content_hash, confirmation.content_hash);
    app::sdlc_routing::validate_routes(&routing.policy.routes).unwrap();
    assert_eq!(
        routing.policy.routing_hash,
        app::sdlc::canonical_hash(&routing.policy.routes).unwrap()
    );
    let mut expected = app::sdlc::analysis_intent(state, confirmation, intent.intent_id).unwrap();
    expected.workflow = routing.policy.routes.analyst.workflow_key;
    assert_eq!(intent, expected);
    let unreserved = tx.query_one(statement(
        "SELECT pm_execution_id IS NULL AND pm_owner_version=0 AND pm_admission_state IS NULL AS value FROM sdlc_tasks WHERE task_id=$1",
        vec![task.into()],
    )).await.unwrap().unwrap().try_get::<bool>("","value").unwrap();
    assert!(unreserved);
    exec(
        tx,
        "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
        vec![task.into(), json_value(state).unwrap()],
    )
    .await
    .unwrap();
    exec(tx,"INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id,created_at)
         SELECT $1,i.id,i.status_id,s.id,u.id,now() FROM issues i
         JOIN statuses s ON s.id=(SELECT id FROM statuses WHERE name='SDLC Analysis' ORDER BY id LIMIT 1)
         JOIN users u ON u.central_sub=$3 WHERE i.id=$2 AND i.status_id<>s.id",
        vec![Uuid::new_v4().into(),task.into(),original.actor.subject.clone().into()]).await.unwrap();
    exec(tx,"UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name='SDLC Analysis' ORDER BY id LIMIT 1),sprint_id=NULL,updated_at=now() WHERE id=$1",
        vec![task.into()]).await.unwrap();
    exec(tx,"INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result) VALUES($1,$2,$3,$4,$5)",
        vec![task.into(),original.actor.subject.clone().into(),original.command.key().into(),original.sha256.clone().into(),original.result.clone().into()]).await.unwrap();
    for (id, event, stage, result) in [
        (
            Uuid::new_v4(),
            "requirements.confirmed",
            Stage::Backlog,
            original.result.clone(),
        ),
        (
            intent.intent_id,
            "analysis.intent_created",
            Stage::Analysis,
            serde_json::to_value(&intent).unwrap(),
        ),
    ] {
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":state.root_task_id,
            "owner_subject":state.owner_subject,"stage":stage,"requirement_revision":state.current_revision(),"result":result});
        exec(
            tx,
            "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
            vec![id.into(), task.into(), event.into(), payload.into()],
        )
        .await
        .unwrap();
    }
}

async fn predecessor_command(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    actor: &Principal,
    command: SdlcCommand,
) -> OriginalCommand {
    app::sdlc::validate_key(command.key()).unwrap();
    let tx = repo.db.begin().await.unwrap();
    let mut state = repo.load(&tx, task, actor).await.unwrap();
    app::sdlc::authorize(&state, actor, &repo.config, &command).unwrap();
    let sha256 = app::sdlc::command_hash(&command).unwrap();
    assert!(
        PostgresSdlcRepository::replay(&tx, task, actor, command.key(), &sha256)
            .await
            .unwrap()
            .is_none()
    );
    let result = app::sdlc::apply(&mut state, actor, &repo.config, &command).unwrap();
    let event = repo
        .persist_result(&tx, task, &command, &result, &state)
        .await
        .unwrap();
    let original = OriginalCommand {
        actor: actor.clone(),
        body: serde_json::to_value(&command).unwrap(),
        command,
        sha256,
        result,
    };
    if event == "requirements.confirmed" {
        // Test-only historical intent, not a current confirmation side effect.
        state.stage = Stage::Analysis;
        let confirmation = state.confirmations.last().unwrap();
        let row = tx
            .query_one(statement(
                "SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1",
                vec![task.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let routing: domain::sdlc_routing::TaskRoutingSnapshot =
            parse(row.try_get("", "payload").unwrap());
        let mut intent = app::sdlc::analysis_intent(&state, confirmation, Uuid::new_v4()).unwrap();
        intent.workflow = routing.policy.routes.analyst.workflow_key;
        exec(&tx,"INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,routing_snapshot_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            vec![intent.intent_id.into(),task.into(),intent.requirement_revision.into(),intent.content_hash.clone().into(),intent.confirmation_id.into(),intent.operation_key.clone().into(),json_value(&intent).unwrap(),routing.snapshot_id.into()]).await.unwrap();
        finish_predecessor_confirmation(&tx, &state, &original).await;
    } else {
        PostgresSdlcRepository::finish(
            &tx,
            task,
            actor,
            original.command.key(),
            &original.sha256,
            &original.result,
            event,
            &state,
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    original
}

async fn seed_legacy_history(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    agent: Uuid,
) -> Vec<OriginalCommand> {
    let owner = actor("owner", true, vec![]);
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: agent,
        version: 1,
        machine_subject: "pm".into(),
    };
    let fence = MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: agent,
        assignment_version: 1,
    };
    let pm = actor("pm", false, vec![assignment.scope(task)]);
    let verifier = actor(
        "verifier",
        false,
        vec![format!("task-tracker:sdlc:evidence:{task}:1")],
    );
    let mut originals = vec![
        predecessor_command(
            repo,
            task,
            &actor("pm", false, vec!["task-tracker:sdlc:assign".into()]),
            SdlcCommand::Assign(AssignCommand {
                assignment,
                expected_assignment_version: None,
                idempotency_key: "upgrade:assign".into(),
            }),
        )
        .await,
    ];
    let mut document: RequirementsDocument = parse(
        json!({"goal":"Preserved legacy goal","scope":["Scope"],
        "exclusions":[],"scenarios":["Scenario"],"acceptance_criteria":["Acceptance"],"constraints":[],
        "dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]}),
    );
    originals.push(
        predecessor_command(
            repo,
            task,
            &pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: fence.clone(),
                expected_requirement_revision: None,
                document: document.clone(),
                idempotency_key: "upgrade:revision:1".into(),
            }),
        )
        .await,
    );
    let question = Uuid::new_v4();
    let option = Uuid::new_v4();
    originals.push(
        predecessor_command(
            repo,
            task,
            &pm,
            SdlcCommand::PublishQuestion(PublishQuestion {
                fence: fence.clone(),
                request_id: Uuid::new_v4(),
                question_id: question,
                expected_question_version: None,
                requirement_revision: 1,
                checkpoint_id: Uuid::new_v4(),
                requirement_reference: Some("synthetic:requirements:1".into()),
                text: "Which retained business scope?".into(),
                rationale: "Preserve actual owner answer history".into(),
                required: true,
                mode: QuestionMode::Single,
                options: vec![QuestionOption {
                    id: option,
                    label: "Confirmed scope".into(),
                    consequences: "Bounded change".into(),
                    is_custom: false,
                }],
                recommended_option_id: None,
                idempotency_key: "upgrade:question".into(),
            }),
        )
        .await,
    );
    originals.push(
        predecessor_command(
            repo,
            task,
            &owner,
            SdlcCommand::Answer {
                question_id: question,
                command: AnswerCommand {
                    expected_question_version: 1,
                    requirement_revision: 1,
                    selected_option_ids: vec![option],
                    text: None,
                    comment: Some("Synthetic actual fixture owner answer".into()),
                    idempotency_key: "upgrade:answer".into(),
                },
            },
        )
        .await,
    );
    document.goal = "Preserved legacy goal after actual owner answer".into();
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    originals.push(
        predecessor_command(
            repo,
            task,
            &pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: fence.clone(),
                expected_requirement_revision: Some(1),
                document,
                idempotency_key: "upgrade:revision:2".into(),
            }),
        )
        .await,
    );
    for check in ["check", "prerequisite"] {
        originals.push(
            predecessor_command(
                repo,
                task,
                &verifier,
                SdlcCommand::Evidence(EvidenceCommand {
                    fence: fence.clone(),
                    requirement_revision: 2,
                    content_hash: hash.clone(),
                    check_id: check.into(),
                    evidence_reference: format!("synthetic:verified:{check}"),
                    idempotency_key: format!("upgrade:evidence:{check}"),
                }),
            )
            .await,
        );
    }
    originals.push(
        predecessor_command(
            repo,
            task,
            &owner,
            SdlcCommand::Confirm {
                revision: 2,
                command: ConfirmCommand {
                    content_hash: hash,
                    idempotency_key: "upgrade:confirm".into(),
                    expected_routing_policy_version: Some(1),
                },
            },
        )
        .await,
    );
    originals
}

async fn assert_original_commands(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    originals: &[OriginalCommand],
) {
    let tx = repo.db.begin().await.unwrap();
    for original in originals {
        assert_eq!(
            serde_json::to_value(&original.command).unwrap(),
            original.body
        );
        assert_eq!(
            app::sdlc::command_hash(&original.command).unwrap(),
            original.sha256
        );
        let decoded: SdlcCommand = serde_json::from_value(original.body.clone()).unwrap();
        assert_eq!(app::sdlc::command_hash(&decoded).unwrap(), original.sha256);
        assert_eq!(
            PostgresSdlcRepository::replay(
                &tx,
                task,
                &original.actor,
                original.command.key(),
                &original.sha256
            )
            .await
            .unwrap(),
            Some(original.result.clone())
        );
        assert!(
            PostgresSdlcRepository::replay(
                &tx,
                task,
                &original.actor,
                original.command.key(),
                &"0".repeat(64)
            )
            .await
            .is_err()
        );
    }
    tx.rollback().await.unwrap();
}

async fn rejected(db: &DatabaseConnection, sql: &str, task: Uuid, message: &str) {
    let tx = db.begin().await.unwrap();
    let error = tx
        .execute(statement(sql, vec![task.into()]))
        .await
        .unwrap_err();
    tx.rollback().await.unwrap();
    assert!(error.to_string().contains(message), "{error}");
}

async fn guard_definition(db: &impl ConnectionTrait, name: &str) -> String {
    db.query_one(statement(
        "SELECT pg_get_functiondef($1::regprocedure) AS definition",
        vec![name.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "definition")
    .unwrap()
}

async fn pending_096_guard_eol_cases(db: &DatabaseConnection) {
    let prefix = concat!(
        include_str!("../../migration/src/single_task_lifecycle.sql"),
        "\n",
        include_str!("../../migration/src/critical_pm_return.sql"),
        "\n",
        include_str!("../../migration/src/critical_pm_review.sql")
    )
    .replace("\r\n", "\n");
    let lifecycle =
        include_str!("../../migration/src/critical_pm_review_lifecycle.sql").replace("\r\n", "\n");
    let (prelude, guarded) = lifecycle
        .split_once("-- Preserve legacy function bodies; checked anchors insert narrow current-transaction branches only.\n")
        .unwrap();
    let first = guarded
        .split_once("-- Cross-table UUID uniqueness")
        .unwrap()
        .0;
    let second = lifecycle
        .split_once("-- Existing PM ownership/write permits are preserved. This exact journal is the ONLY new branch.\n")
        .unwrap().1
        .split_once("-- Keep every legacy guard condition").unwrap().0;
    let names = [
        "sdlc_delivery_canonical_gate()",
        "sdlc_critical_projection_gate()",
        "sdlc_pm_reservation_gate()",
    ];
    for eol in ["\n", "\r\n"] {
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&prefix.replace('\n', eol))
            .await
            .unwrap();
        tx.execute_unprepared(&prelude.replace('\n', eol))
            .await
            .unwrap();
        let mut definitions = Vec::new();
        for name in names {
            // Synthetic predecessor representation only; all DDL is rolled back.
            let definition = guard_definition(&tx, name)
                .await
                .replace("\r\n", "\n")
                .replace('\n', eol);
            tx.execute_unprepared(&definition).await.unwrap();
            definitions.push(guard_definition(&tx, name).await);
        }
        for (index, name) in names.iter().enumerate() {
            let anchor = format!("BEGIN{eol}");
            for mutation in ["missing", "duplicate", "mixed"] {
                let check = tx.begin().await.unwrap();
                let replacement = match mutation {
                    "missing" => format!("begin{eol}"),
                    "duplicate" => format!("-- BEGIN{eol}{anchor}"),
                    _ => format!(
                        "-- BEGIN{}{anchor}",
                        if eol == "\n" { "\r\n" } else { "\n" }
                    ),
                };
                let changed = definitions[index].replacen(&anchor, &replacement, 1);
                assert_ne!(changed, definitions[index]);
                check.execute_unprepared(&changed).await.unwrap();
                let error = check
                    .execute_unprepared(if index == 2 { second } else { first })
                    .await
                    .unwrap_err();
                assert!(
                    error.to_string().contains(if index == 2 {
                        "SDLC PM ownership guard anchor differs"
                    } else {
                        "SDLC guard anchor differs"
                    }),
                    "{name}: {mutation}: {error}"
                );
                check.rollback().await.unwrap();
            }
        }
        tx.execute_unprepared(first).await.unwrap();
        tx.execute_unprepared(second).await.unwrap();
        for (index, name) in names.iter().enumerate() {
            let after = guard_definition(&tx, name).await;
            let anchor = format!("BEGIN{eol}");
            let (before, tail) = definitions[index].split_once(&anchor).unwrap();
            let inserted = after
                .strip_prefix(before)
                .unwrap()
                .strip_suffix(tail)
                .unwrap();
            assert!(inserted.starts_with(&anchor));
            assert!(inserted.contains("sdlc_critical_pm_exact_event"));
            assert_eq!(inserted.contains("\r\n"), eol == "\r\n");
        }
        tx.rollback().await.unwrap();
        // Also exercise the complete unchanged 0096 SQL sequence in this EOL representation.
        let complete = db.begin().await.unwrap();
        complete
            .execute_unprepared(&format!("{prefix}\n{lifecycle}").replace('\n', eol))
            .await
            .unwrap();
        complete.rollback().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_DRAFT_FLOW_TEST_DATABASE_URL ending in _draft_flow_test"]
async fn populated_0033_through_095_to_096_preserves_original_ids_hashes_and_history() {
    let names: Vec<String> = migration::Migrator::migrations()
        .iter()
        .map(|m| m.name().to_owned())
        .collect();
    let accepted = names
        .iter()
        .position(|n| n == "m20260918_0000033_central_subject")
        .unwrap()
        + 1;
    let predecessor = names
        .iter()
        .position(|n| n == "m20261012_0000095_pm_completion")
        .unwrap()
        + 1;
    assert_eq!((accepted, predecessor, names.len()), (19, 26, 34));
    assert_eq!(names[predecessor], "m20261013_0000096_single_task_delivery");
    assert_eq!(
        names[predecessor + 1],
        "m20261017_0000097_task_source_binding"
    );
    assert_eq!(
        names[predecessor + 2],
        "m20261018_0000098_pm_unsubmitted_recovery"
    );
    assert_eq!(
        names[predecessor + 3],
        "m20261018_0000098_project_repository_catalog"
    );
    assert_eq!(
        names[predecessor + 4],
        "m20261021_0000099_pm_bootstrap_retry"
    );
    assert_eq!(
        &names[31..],
        &[
            "m20261019_0000099_unified_spaces",
            "m20261021_0000100_human_sdlc_spaces",
            "m20261021_0000101_native_sdlc_spaces",
        ]
    );
    let url = std::env::var("TT_DRAFT_FLOW_TEST_DATABASE_URL")
        .expect("fresh isolated upgrade database required");
    let db = Database::connect(&url).await.unwrap();
    let row = db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_draft_flow_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, Some(accepted.try_into().unwrap()))
        .await
        .unwrap();
    let ledger_0033 = ledger(&db, &names[..accepted]).await;
    assert!(tables(&db).await.is_empty(), "0033 has no SDLC storage");
    let original = OriginalData::seed(&db).await;
    let business_0033 = original.snapshot(&db, false).await;
    for (field, expected) in [
        ("issues", 2),
        ("comments", 2),
        ("custom_fields", 1),
        ("custom_values", 2),
        ("status_history", 2),
    ] {
        assert_eq!(
            business_0033[field].as_array().unwrap().len(),
            expected,
            "{field}"
        );
    }
    assert_eq!(count(&db, "issues").await, 2);
    assert_eq!(
        business_0033["custom_values"][0]["value"]["chat_ref"],
        json!(original.chat_ref)
    );
    assert_eq!(
        business_0033["custom_values"][0]["value"]["requirements_ref"],
        json!(original.requirements_ref)
    );
    assert_eq!(
        business_0033["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| !i["deleted_at"].is_null())
            .count(),
        1
    );

    migration::Migrator::up(&db, Some((predecessor - accepted).try_into().unwrap()))
        .await
        .unwrap();
    let ledger_095 = ledger(&db, &names[..predecessor]).await;
    for entry in ledger_0033.as_array().unwrap() {
        assert!(ledger_095.as_array().unwrap().contains(entry));
    }
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    let namespace = db.query_one(statement("SELECT namespace_managed,high_water_mark FROM projects JOIN project_issue_counters ON project_id=id WHERE id=$1",vec![original.project.into()])).await.unwrap().unwrap();
    assert!(!namespace.try_get::<bool>("", "namespace_managed").unwrap());
    assert_eq!(namespace.try_get::<i64>("", "high_water_mark").unwrap(), 41);
    assert!(scalar_json(&db,"SELECT to_jsonb(to_regclass('public.sdlc_routing_snapshot_inventory') IS NULL) AS value",vec![]).await.as_bool().unwrap());
    let repo = PostgresSdlcRepository::connect(&url, config(original.project, false))
        .await
        .unwrap();
    let owner = actor("owner", true, vec![]);
    let create = create("upgrade:original:draft", None);
    let create_body = serde_json::to_value(&create).unwrap();
    assert!(create_body.get("flow").is_none());
    let (draft, replayed) = repo
        .create_draft(original.project, &owner, create.clone())
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(
        draft.task_key, "UPG-42",
        "trashed issue number must remain consumed"
    );
    let policy_command = SetRoutingPolicy {
        expected_version: None,
        routes: routes(),
        idempotency_key: "upgrade:routing:original".into(),
    };
    let policy_body = serde_json::to_value(&policy_command).unwrap();
    let (policy, replayed) = repo
        .set_routing_policy(original.project, &owner, policy_command.clone())
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(policy.contract_version, 1);
    let originals = Box::pin(seed_legacy_history(
        &repo,
        draft.task_id,
        policy.routes.project_manager.agent_id,
    ))
    .await;
    assert_eq!(originals.len(), 8);
    assert_original_commands(&repo, draft.task_id, &originals).await;
    let old_tables = tables(&db).await;
    let history_095 = history(&db, &old_tables, false).await;
    for (name, expected) in [
        ("sdlc_tasks", 1),
        ("sdlc_draft_creations", 1),
        ("sdlc_assignments", 1),
        ("sdlc_agent_bindings", 1),
        ("sdlc_requirements", 2),
        ("sdlc_requests", 1),
        ("sdlc_question_versions", 1),
        ("sdlc_options", 1),
        ("sdlc_answers", 1),
        ("sdlc_evidence", 2),
        ("sdlc_confirmations", 1),
        ("sdlc_project_routing_revisions", 1),
        ("sdlc_project_routing_heads", 1),
        ("sdlc_project_routing_operations", 1),
        ("sdlc_task_routing_snapshots", 1),
        ("sdlc_analysis_intents", 1),
        ("sdlc_idempotency", 8),
        ("sdlc_outbox", 10),
        ("sdlc_pm_executions", 0),
        ("sdlc_pm_execution_leases", 0),
        ("sdlc_pm_native_admissions", 0),
        ("sdlc_pm_completions", 0),
        ("sdlc_analysis_reservations", 0),
    ] {
        assert_eq!(
            history_095[name].as_array().unwrap().len(),
            expected,
            "{name}"
        );
    }
    let creation = &history_095["sdlc_draft_creations"][0];
    let input_ref: Uuid = serde_json::from_value(creation["input_snapshot_ref"].clone()).unwrap();
    assert!(!input_ref.is_nil());
    assert_eq!(creation["input_title"], create_body["title"]);
    assert_eq!(creation["input_description"], create_body["description"]);
    assert_eq!(
        creation["input_sha256"],
        json!(app::sdlc::pm_draft_input_hash(&create.title, &create.description).unwrap())
    );
    assert_eq!(
        creation["payload_hash"],
        json!(
            app::sdlc::canonical_hash(&json!({"operation":"create_draft","payload":create_body}))
                .unwrap()
        )
    );
    assert_eq!(
        history_095["sdlc_project_routing_operations"][0]["payload_hash"],
        json!(app::sdlc::canonical_hash(&policy_body).unwrap())
    );
    let original_snapshot = &history_095["sdlc_task_routing_snapshots"][0];
    let original_intent = &history_095["sdlc_analysis_intents"][0];
    assert_eq!(
        original_intent["routing_snapshot_id"],
        original_snapshot["snapshot_id"]
    );
    assert_eq!(original_intent["requirement_revision"], json!(2));
    assert_eq!(
        original_intent["confirmation_id"],
        history_095["sdlc_confirmations"][0]["id"]
    );
    let binding = &history_095["sdlc_tasks"][0];
    assert!(binding["pm_execution_id"].is_null());
    assert_eq!(binding["pm_owner_version"], json!(0));
    assert_eq!(binding["state"]["stage"], json!("Analysis"));
    for absent in ["flow", "status", "lifecycle"] {
        assert!(binding["state"].get(absent).is_none());
    }
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    assert_eq!(count(&db, "issues").await, 3);
    let legacy_projection = legacy_history_snapshot(&db, draft.task_id).await;

    let retrofit_flow = "UPDATE sdlc_tasks SET state=state||'{\"flow\":\"single-task-v1\"}'::jsonb WHERE task_id=$1";
    rejected(
        &db,
        retrofit_flow,
        draft.task_id,
        "queued Analysis requires guarded admission before lifecycle mutation",
    )
    .await;

    Box::pin(pending_096_guard_eol_cases(&db)).await;
    assert_eq!(ledger(&db, &names[..predecessor]).await, ledger_095);
    assert_eq!(history(&db, &old_tables, false).await, history_095);
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    migration::Migrator::up(&db, Some(1)).await.unwrap();
    let ledger_096 = ledger(&db, &names[..predecessor + 1]).await;
    for entry in ledger_095.as_array().unwrap() {
        assert!(ledger_096.as_array().unwrap().contains(entry));
    }
    let tables_096 = tables(&db).await;
    let history_096 = history(&db, &tables_096, false).await;
    migration::Migrator::up(&db, Some(1)).await.unwrap();
    let ledger_097 = ledger(&db, &names[..predecessor + 2]).await;
    for entry in ledger_096.as_array().unwrap() {
        assert!(ledger_097.as_array().unwrap().contains(entry));
    }
    assert_eq!(history(&db, &tables_096, false).await, history_096);
    for name in [
        "sdlc_source_policy_revisions",
        "sdlc_source_policy_heads",
        "sdlc_source_policy_operations",
        "sdlc_task_source_selections",
        "sdlc_draft_source_enrollments",
        "sdlc_source_ref_owners",
        "sdlc_task_source_bindings",
    ] {
        assert_eq!(count(&db, name).await, 0, "097 must not backfill {name}");
    }
    assert_eq!(history(&db, &old_tables, true).await, history_095);
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    assert_eq!(
        legacy_history_snapshot(&db, draft.task_id).await,
        legacy_projection
    );
    let all_tables = tables(&db).await;
    let new_tables: Vec<String> = all_tables
        .iter()
        .filter(|t| !old_tables.contains(t))
        .cloned()
        .collect();
    for required in [
        "sdlc_critical_blockers",
        "sdlc_critical_pm_returns",
        "sdlc_critical_pm_reviews",
        "sdlc_critical_pm_lifecycle_operations",
        "sdlc_task_routing_generations",
    ] {
        assert!(
            new_tables.iter().any(|t| t == required),
            "missing096 table {required}"
        );
    }
    for name in &new_tables {
        // Status mapping is the sole new seeded reference table, not synthesized history.
        if name != "sdlc_issue_status_mapping" {
            assert_eq!(count(&db, name).await, 0, "{name}");
        }
    }
    assert_eq!(count(&db, "sdlc_issue_status_mapping").await, 27);
    let intent_096 = scalar_json(
        &db,
        "SELECT to_jsonb(i) AS value FROM sdlc_analysis_intents i WHERE task_id=$1",
        vec![draft.task_id.into()],
    )
    .await;
    assert_eq!(intent_096.get("routing_generation"), Some(&Json::Null));
    assert_eq!(
        intent_096.get("review_routing_snapshot_id"),
        Some(&Json::Null)
    );
    assert_eq!(
        intent_096["routing_snapshot_id"],
        original_intent["routing_snapshot_id"]
    );
    let inventory = scalar_json(
        &db,
        "SELECT to_jsonb(s) AS value FROM sdlc_routing_snapshot_inventory s WHERE task_id=$1",
        vec![draft.task_id.into()],
    )
    .await;
    let mut expected_inventory = original_snapshot.clone();
    expected_inventory
        .as_object_mut()
        .unwrap()
        .insert("generation".into(), json!(1));
    assert_eq!(inventory, expected_inventory);
    assert_eq!(count(&db, "sdlc_routing_snapshot_inventory").await, 1);
    let read_snapshot = repo
        .task_routing_snapshot(draft.task_id, &owner)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(read_snapshot).unwrap(),
        original_snapshot["payload"]
    );
    let read_intent = repo.analysis_intent(draft.task_id, &owner).await.unwrap();
    assert_eq!(
        serde_json::to_value(read_intent).unwrap(),
        original_intent["payload"]
    );
    assert_original_commands(&repo, draft.task_id, &originals).await;
    let (replayed_draft, replayed) = repo
        .create_draft(original.project, &owner, create.clone())
        .await
        .unwrap();
    assert!(replayed);
    assert_eq!(replayed_draft, draft);
    let (replayed_policy, replayed) = repo
        .set_routing_policy(original.project, &owner, policy_command.clone())
        .await
        .unwrap();
    assert!(replayed);
    assert_eq!(replayed_policy, policy);
    let mut changed = create;
    changed.description.push_str(" changed");
    assert!(
        repo.create_draft(original.project, &owner, changed)
            .await
            .is_err()
    );
    let mut changed = policy_command;
    changed.routes.project_manager.fleet_config_revision += 1;
    assert!(
        repo.set_routing_policy(original.project, &owner, changed)
            .await
            .is_err()
    );
    rejected(
        &db,
        "UPDATE sdlc_requirements SET payload=payload WHERE task_id=$1",
        draft.task_id,
        "append-only",
    )
    .await;
    rejected(
        &db,
        "DELETE FROM sdlc_task_routing_snapshots WHERE task_id=$1",
        draft.task_id,
        "append-only",
    )
    .await;
    // 096 replaces the Analysis gate; the same retrofit remains forbidden
    // before it can reach the Draft flow guard, without a result or write permit.
    rejected(
        &db,
        retrofit_flow,
        draft.task_id,
        "SDLC legacy Analysis requires exact accepted result and permit",
    )
    .await;
    assert_eq!(history(&db, &old_tables, true).await, history_095);
    assert_eq!(
        legacy_history_snapshot(&db, draft.task_id).await,
        legacy_projection
    );
    let before_pm_recovery = history(&db, &all_tables, false).await;
    migration::Migrator::up(&db, Some(1)).await.unwrap();
    let ledger_pm_recovery = ledger(&db, &names[..predecessor + 3]).await;
    for entry in ledger_097.as_array().unwrap() {
        assert!(ledger_pm_recovery.as_array().unwrap().contains(entry));
    }
    assert_eq!(tables(&db).await, all_tables);
    assert_eq!(history(&db, &all_tables, false).await, before_pm_recovery);
    assert_eq!(scalar_json(&db,
        "SELECT to_jsonb(count(*)) AS value FROM sdlc_pm_lease_operations WHERE recovery IS NOT NULL",
        vec![]).await, json!(0));
    let error = migration::Migrator::down(&db, Some(1)).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("PM recovery history is immutable; destructive rollback is disabled")
    );
    assert_eq!(
        ledger(&db, &names[..predecessor + 3]).await,
        ledger_pm_recovery
    );
    assert_eq!(history(&db, &all_tables, false).await, before_pm_recovery);
    let before_catalog = history(&db, &all_tables, false).await;
    migration::Migrator::up(&db, Some(1)).await.unwrap();
    let ledger_catalog = ledger(&db, &names[..predecessor + 4]).await;
    for entry in ledger_pm_recovery.as_array().unwrap() {
        assert!(ledger_catalog.as_array().unwrap().contains(entry));
    }
    assert_eq!(tables(&db).await, all_tables);
    assert_eq!(history(&db, &all_tables, false).await, before_catalog);
    assert_eq!(count(&db, "project_repository_links").await, 0);
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    let error = migration::Migrator::down(&db, Some(1)).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("project repository associations must be preserved")
    );
    assert_eq!(ledger(&db, &names[..predecessor + 4]).await, ledger_catalog);
    assert_eq!(history(&db, &all_tables, false).await, before_catalog);
    let before_retry = history(&db, &all_tables, false).await;
    migration::Migrator::up(&db, Some(1)).await.unwrap();
    let ledger_retry = ledger(&db, &names).await;
    for entry in ledger_catalog.as_array().unwrap() {
        assert!(ledger_retry.as_array().unwrap().contains(entry));
    }
    assert_eq!(tables(&db).await, all_tables);
    assert_eq!(history(&db, &all_tables, false).await, before_retry);
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    assert_eq!(scalar_json(&db,
        "SELECT to_jsonb(count(*)) AS value FROM pg_indexes WHERE schemaname='public' AND indexname IN ('sdlc_pm_bootstrap_retry_original','sdlc_pm_bootstrap_retry_successor')",
        vec![]).await, json!(2));
    let error = migration::Migrator::down(&db, Some(1)).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("PM bootstrap retry history is immutable; destructive rollback is disabled")
    );
    assert_eq!(ledger(&db, &names).await, ledger_retry);
    assert_eq!(history(&db, &all_tables, false).await, before_retry);
    let before_rerun = history(&db, &all_tables, false).await;
    migration::Migrator::up(&db, None).await.unwrap();
    assert_eq!(ledger(&db, &names).await, ledger_retry);
    assert_eq!(history(&db, &all_tables, false).await, before_rerun);
    assert_eq!(original.snapshot(&db, true).await, business_0033);
    assert_eq!(count(&db, "issues").await, 3);
    assert_eq!(count(&db, "comments").await, 2);
    assert_eq!(count(&db, "issue_status_history").await, 5);
}
