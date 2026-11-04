//! Register as a child of the existing candidates PG fixture; no new runner/database setup.
//! All Fleet/native/Workflow terminal observations here are explicitly synthetic.
use super::*;
use crate::sdlc::attachment_artifacts as artifacts;
use domain::attachment_artifact::{ArtifactUpload, Seal};
use domain::sdlc_source::TaskSourceRepository;

const PRODUCER: &str = "88888888-8888-4888-8888-888888888888";
const CONSUMER: &str = "99999999-9999-4999-8999-999999999999";

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn related_discussion_keeps_assignment_project_and_pagination_boundaries() {
    use domain::sdlc_discussion::{DiscussionQuery, DiscussionRepository};
    let f = Fixture::new().await;
    let user = Uuid::new_v4();
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,'research@example.test','research','Research','!',$2,true)",vec![user.into(),PRODUCER.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![f.project.into(), user.into()],
    )
    .await
    .unwrap();
    let source = f.queued_analysis("research-source", true).await;
    bind(&f, source, "research-source").await;
    let related = f
        .draft("related-source", Some(DraftFlow::SingleTaskV1))
        .await;
    let unrelated = f
        .draft("unrelated-source", Some(DraftFlow::SingleTaskV1))
        .await;
    let foreign_project = Uuid::new_v4();
    let foreign_board = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    exec(&f.db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) SELECT $1,'OTHER','Other',owner_id,$3 FROM projects WHERE id=$2",vec![foreign_project.into(),f.project.into(),foreign_board.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO boards(id,project_id,name,columns) VALUES($1,$2,'Other','[]')",
        vec![foreign_board.into(), foreign_project.into()],
    )
    .await
    .unwrap();
    exec(&f.db,"INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,description,reporter_id,priority,labels,position,time_spent_seconds,created_at,updated_at) SELECT $1,$2,'OTHER-1',issue_type,status_id,'Foreign source','Foreign text',reporter_id,priority,labels,position,time_spent_seconds,created_at,updated_at FROM issues WHERE id=$3",vec![foreign.into(),foreign_project.into(),related.into()]).await.unwrap();
    // Both link directions are sources; a real foreign link must stay invisible.
    for (left, right, kind) in [
        (related, source, "relates"),
        (source, related, "blocks"),
        (source, foreign, "relates"),
    ] {
        exec(
            &f.db,
            "INSERT INTO issue_links(id,source_id,target_id,link_type) VALUES($1,$2,$3,$4)",
            vec![
                Uuid::new_v4().into(),
                left.into(),
                right.into(),
                kind.into(),
            ],
        )
        .await
        .unwrap();
    }
    for body in ["Related source first page", "Related source second page"] {
        exec(&f.db,"INSERT INTO comments(id,issue_id,author_id,body,created_at,updated_at) VALUES($1,$2,$3,$4,clock_timestamp(),clock_timestamp())",vec![Uuid::now_v7().into(),related.into(),user.into(),body.into()]).await.unwrap();
    }
    let producer = actor(
        PRODUCER,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    let own = f
        .repo
        .discussion(source, &producer, DiscussionQuery::default())
        .await
        .unwrap();
    assert_eq!(own.source_task_id, source);
    assert_eq!(own.task_id, source);
    assert_eq!(own.related_tasks.len(), 1);
    assert_eq!(own.related_tasks[0].task_id, related);
    let query = DiscussionQuery {
        related_task: Some(related),
        limit: Some(1),
        ..Default::default()
    };
    let first = f.repo.discussion(source, &producer, query).await.unwrap();
    assert_eq!(first.source_task_id, source);
    assert_eq!(first.task_id, related);
    assert_eq!(first.title, "related-source");
    assert_eq!(first.comments.len(), 1);
    let second = f
        .repo
        .discussion(
            source,
            &producer,
            DiscussionQuery {
                related_task: Some(related),
                limit: Some(1),
                snapshot: first.snapshot,
                after: first.next_after,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.comments.len(), 1);
    assert_ne!(first.comments[0].id, second.comments[0].id);
    assert!(second.next_after.is_none());
    for target in [unrelated, foreign] {
        assert!(matches!(
            f.repo
                .discussion(
                    source,
                    &producer,
                    DiscussionQuery {
                        related_task: Some(target),
                        ..Default::default()
                    }
                )
                .await,
            Err(AppError::Forbidden)
        ));
    }
    assert!(
        f.repo
            .discussion(related, &producer, DiscussionQuery::default())
            .await
            .is_err()
    );
    exec(
        &f.db,
        "UPDATE issues SET deleted_at=clock_timestamp() WHERE id=$1",
        vec![related.into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        f.repo
            .discussion(
                source,
                &producer,
                DiscussionQuery {
                    related_task: Some(related),
                    ..Default::default()
                }
            )
            .await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![f.project.into(), user.into()],
    )
    .await
    .unwrap();
    assert!(
        f.repo
            .discussion(source, &producer, DiscussionQuery::default())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn native_artifact_content_port_is_exact_and_rechecked_at_commit() {
    let f = Fixture::new().await;
    let author = Uuid::new_v4();
    exec(&f.db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,'native-artifact@example.test','native-artifact','Native artifact','!',$2,true)",vec![author.into(),PRODUCER.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![f.project.into(), author.into()],
    )
    .await
    .unwrap();
    let task = f.queued_analysis("native-artifact", true).await;
    let head = bind(&f, task, "native-artifact").await;
    let assignment = head.assignment.as_ref().unwrap();
    let seal = Seal::new(
        task,
        domain::attachment_artifact::ArtifactBinding {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            role: assignment.role,
        },
        head.binding.as_ref().unwrap().run_id,
        "analysis.md",
        "text/markdown",
        "a".repeat(64),
    )
    .unwrap();
    exec(&f.db,"INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state,operation_id,metadata) SELECT registry_instance_id,namespace_id,1,'active',$2,'{}'::jsonb FROM tracker_namespace_bindings WHERE resource_id=$1",vec![f.project.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&f.db,"INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) SELECT resource_id,registry_instance_id,namespace_id FROM tracker_namespace_bindings",vec![]).await.unwrap();
    exec(
        &f.db,
        "UPDATE tracker_space_admission_state SET active=true WHERE singleton",
        vec![],
    )
    .await
    .unwrap();
    let metadata = json!({"id":seal.artifact_id,"issue_id":task,"author_id":author,"file_name":"analysis.md","content_type":"text/markdown","size_bytes":10,"storage_key":seal.key()});
    for variation in [
        "missing-port",
        "foreign-task",
        "foreign-author",
        "changed-file",
        "ordinary-key",
        "read-only",
        "expired-commit",
        "valid",
    ] {
        let tx = f.db.begin().await.unwrap();
        let now = chrono::Utc::now().timestamp();
        let caller = json!({"schema_version":1,"identity":{"issuer":"http://auth.example.test","subject":PRODUCER},"validity":{"observed_at":now,"valid_until":now+60},"scopes":["task-tracker:read","task-tracker:write"],"writable":variation!="read-only"});
        tx.execute(statement(
            "SELECT set_config('pdlc.tracker.native_caller',$1,true)",
            vec![caller.to_string().into()],
        ))
        .await
        .unwrap();
        let scope = json!({"schema_version":1,"task_id":task,"project_id":f.project,"identity":caller["identity"],"validity":caller["validity"],"writable":caller["writable"]});
        tx.execute(statement(
            "SELECT tracker_bind_native_task($1)",
            vec![scope.into()],
        ))
        .await
        .unwrap();
        let mut desired = metadata.clone();
        match variation {
            "foreign-task" => desired["issue_id"] = json!(Uuid::new_v4()),
            "foreign-author" => desired["author_id"] = json!(Uuid::new_v4()),
            "changed-file" => desired["file_name"] = json!("changed.md"),
            "ordinary-key" => desired["storage_key"] = json!("ordinary.md"),
            _ => (),
        }
        if variation != "missing-port" {
            let capability = if variation == "ordinary-key" {
                &desired
            } else {
                &metadata
            };
            tx.execute(statement(
                "SELECT set_config('pdlc.tracker.native_artifact',$1,true)",
                vec![capability.to_string().into()],
            ))
            .await
            .unwrap();
        }
        let inserted = tx.execute(statement("INSERT INTO attachments(id,issue_id,author_id,file_name,content_type,size_bytes,storage_key,created_at) SELECT id,issue_id,author_id,file_name,content_type,size_bytes,storage_key,now() FROM jsonb_populate_record(NULL::attachments,$1)",vec![desired.into()])).await;
        if variation == "valid" || variation == "expired-commit" {
            inserted.unwrap();
            if variation == "expired-commit" {
                let mut expired = caller.clone();
                expired["validity"]["valid_until"] = json!(now - 1);
                tx.execute(statement(
                    "SELECT set_config('pdlc.tracker.native_caller',$1,true)",
                    vec![expired.to_string().into()],
                ))
                .await
                .unwrap();
                assert!(
                    tx.commit()
                        .await
                        .unwrap_err()
                        .to_string()
                        .contains("native_request_authority_expired")
                );
            } else {
                tx.commit().await.unwrap();
            }
        } else {
            let error = inserted.unwrap_err().to_string();
            assert!(
                error.contains("native_content_")
                    || error.contains("native_sealed_artifact_required"),
                "{variation}: {error}"
            );
            tx.rollback().await.unwrap();
        }
    }
}

/// Operator-only export for standalone CLI/Hermes QA. Never called by the server.
/// The input/output paths and fresh disposable database are explicit opt-ins.
#[tokio::test]
#[ignore = "operator fixture export requires TT_ANALYST_TOOLS_INPUT and a fresh candidates test DB"]
async fn prepare_standalone_analyst_tool_fixture() {
    let input_path = std::env::var("TT_ANALYST_TOOLS_INPUT").expect("explicit fixture input");
    let output_path = std::env::var("TT_ANALYST_TOOLS_OUTPUT").expect("explicit fixture output");
    let input: Json = serde_json::from_slice(&std::fs::read(input_path).unwrap()).unwrap();
    let owner_subject = input["owner_subject"].as_str().unwrap();
    Uuid::parse_str(owner_subject).unwrap();
    let tracker_instance = input["tracker_instance_id"].as_str().unwrap();
    assert_eq!(
        Uuid::parse_str(tracker_instance).unwrap().to_string(),
        tracker_instance
    );
    let mut f = Fixture::new_with_identity(None, owner_subject, tracker_instance).await;
    let subject = input["producer_subject"].as_str().unwrap();
    Uuid::parse_str(subject).unwrap();
    let scheduler = input["scheduler_subject"].as_str().unwrap();
    Uuid::parse_str(scheduler).unwrap();
    exec(
        &f.db,
        "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
         VALUES($1,$2,$3,'Standalone QA scheduler','!',$3,true)",
        vec![
            Uuid::new_v4().into(),
            format!("{scheduler}@example.test").into(),
            scheduler.into(),
        ],
    )
    .await
    .unwrap();
    exec(&f.db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'member' FROM users WHERE central_sub=$2",
        vec![f.project.into(),scheduler.into()]).await.unwrap();
    f.writer.subject = scheduler.into();
    f.reader.subject = scheduler.into();
    f.repo.config.reservation_scheduler_subject = scheduler.into();
    f.roles = parse(input["routes"].clone());
    f.repo
        .set_routing_policy(
            f.project,
            &f.owner,
            SetRoutingPolicy {
                expected_version: Some(1),
                routes: f.roles.clone(),
                idempotency_key: "standalone:routes".into(),
            },
        )
        .await
        .unwrap();
    let namespace: shared::resource_context::NamespaceRef = parse(input["namespace"].clone());
    exec(
        &f.db,
        "UPDATE tracker_namespace_bindings SET registry_instance_id=$2,namespace_id=$3,
         command=jsonb_set(command,'{namespace}',$4) WHERE resource_id=$1",
        vec![
            f.project.into(),
            namespace.registry_instance_id.into(),
            namespace.namespace_id.into(),
            json!(namespace).into(),
        ],
    )
    .await
    .unwrap();
    let source: domain::sdlc_source::SourcePolicySpec = parse(input["source"].clone());
    let verified: domain::sdlc_source::VerifiedSource = parse(json!({
        "namespace":namespace,"forge_instance_id":source.forge_instance_id,
        "repository_id":source.repository_id,"public_name":input["repository"],
        "base_ref":source.base_ref,"base_commit":source.base_commit,
        "default_ref":"refs/heads/main","protected_refs":[],"observed_at":chrono::Utc::now()
    }));
    f.repo
        .set_source_policy(
            f.project,
            &f.owner,
            domain::sdlc_source::SetSourcePolicy {
                expected_source_revision: Some(1),
                source,
                idempotency_key: "standalone:source".into(),
            },
            verified,
        )
        .await
        .unwrap();
    exec(
        &f.db,
        "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
         VALUES($1,$2,$3,'Standalone Analyst QA','!',$3,true)",
        vec![
            Uuid::new_v4().into(),
            format!("{subject}@example.test").into(),
            subject.into(),
        ],
    )
    .await
    .unwrap();
    exec(&f.db,
        "INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'member' FROM users WHERE central_sub=$2",
        vec![f.project.into(),subject.into()],
    ).await.unwrap();
    let mut cases = json!({});
    let ordinal_base = input["ordinal_base"].as_i64().unwrap();
    assert!((1..=1_000_000_000).contains(&ordinal_base));
    f.db.query_one(statement(
        "SELECT setval(pg_get_serial_sequence('sdlc_pm_executions','ordinal'),$1,false)",
        vec![ordinal_base.into()],
    ))
    .await
    .unwrap();
    let name = input["scenario"].as_str().unwrap();
    assert!(["complete", "partial", "missing"].contains(&name));
    let task = f
        .draft(&format!("standalone-{name}"), Some(DraftFlow::SingleTaskV1))
        .await;
    let document: RequirementsDocument = parse(input["requirements_by_case"][name].clone());
    let (pm, _) = crate::sdlc::configured_test_support::confirmed_pm(
        &f.repo,
        task,
        &f.owner,
        f.roles.project_manager.agent_id,
        document,
        false,
    )
    .await;
    // The historical PM receipt is explicitly synthetic, as in existing PG fixtures.
    let mut no_automatic = f.repo.config.clone();
    no_automatic.single_task_project_ids.remove(&f.project);
    let repo = PostgresSdlcRepository {
        db: crate::guarded_db::GuardedDb::new(
            Database::connect(std::env::var("TT_CANDIDATES_TEST_DATABASE_URL").unwrap())
                .await
                .unwrap(),
        ),
        config: no_automatic,
        file_storage: None,
    };
    let pm_actor = actor(
        &pm.assignment.machine_subject,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    crate::sdlc::configured_test_support::complete_pm(&repo, task, &f.owner, &pm_actor, &pm).await;
    seed_analysis_fixture(&f.db, task, &f.owner).await;
    let receipt = f
        .repo
        .lifecycle_read(task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let (reserved, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Reserve {
                expected_task_version: receipt.task.version,
                machine_subject: Some(subject.into()),
                idempotency_key: format!("standalone:reserve:{name}"),
            },
        )
        .await
        .unwrap();
    let (bound, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&reserved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: subject.into(),
                    dispatch_receipt: ArtifactReceipt {
                        reference: "fixture://standalone-no-fleet".into(),
                        sha256: "c".repeat(64),
                    },
                },
                idempotency_key: format!("standalone:bind:{name}"),
            },
        )
        .await
        .unwrap();
    let state = f.repo.read(task, &f.owner).await.unwrap();
    cases[name] = json!({"task_id":task,"receipt":bound,"state":state,
            "workflow_task_key":state.retained_fields["workflow_task_key"]});
    let foreign_project = Uuid::new_v4();
    let foreign_board = Uuid::new_v4();
    exec(
        &f.db,
        "INSERT INTO projects(id,key,name,owner_id,default_board_id)
        SELECT $1,'OTHER','Foreign standalone QA',owner_id,$2 FROM projects WHERE id=$3",
        vec![
            foreign_project.into(),
            foreign_board.into(),
            f.project.into(),
        ],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "INSERT INTO boards(id,project_id,name,columns) VALUES($1,$2,'Foreign QA','[]')",
        vec![foreign_board.into(), foreign_project.into()],
    )
    .await
    .unwrap();
    f.repo
        .config
        .single_task_project_ids
        .insert(foreign_project);
    crate::sdlc::source::configure_test_source(&f.repo, foreign_project, &f.owner).await;
    f.repo
        .set_routing_policy(
            foreign_project,
            &f.owner,
            SetRoutingPolicy {
                expected_version: None,
                routes: f.roles.clone(),
                idempotency_key: "standalone:foreign-routing".into(),
            },
        )
        .await
        .unwrap();
    let (foreign, _) = f
        .repo
        .create_draft(
            foreign_project,
            &f.owner,
            CreateDraftCommand {
                interaction_protocol: Some(InteractionProtocol::ConfiguredCliV1),
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Foreign standalone fixture".into(),
                description: "No Analyst project membership".into(),
                idempotency_key: "standalone:foreign".into(),
            },
        )
        .await
        .unwrap();
    let foreign = foreign.task_id;
    let output = json!({"scope":"synthetic operator preparation; no Fleet/native/PM acceptance",
        "project_id":f.project,"owner_subject":owner_subject,"instance_id":f.repo.config.instance_id,"cases":cases,"foreign_task_id":foreign});
    std::fs::write(output_path, serde_json::to_vec_pretty(&output).unwrap()).unwrap();
}

fn upload(bytes: &[u8], name: &str) -> ArtifactUpload {
    ArtifactUpload {
        file_name: name.into(),
        content_type: "application/json".into(),
        bytes: bytes.to_vec(),
    }
}
fn delivery_fence(receipt: &LifecycleReceipt) -> DeliveryFence {
    let assignment = receipt.assignment.as_ref().unwrap();
    DeliveryFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        fencing_token: assignment.fencing_token,
        lease_id: assignment.lease_id,
        expected_lease_version: receipt.lease.as_ref().unwrap().version,
    }
}

#[tokio::test]
#[ignore = "requires disposable TT_CANDIDATES_TEST_DATABASE_URL; synthetic Workflow receipt, no Complete"]
async fn analyst_tool_proposal_is_replayed_bound_and_comment_checked_without_complete() {
    use domain::sdlc_discussion::{DiscussionRepository, PublishStageComment};
    let f = Fixture::new().await;
    let user = Uuid::new_v4();
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,'proposal-tools@example.test','proposal-tools','Proposal tools','!',$2,true)",vec![user.into(),PRODUCER.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![f.project.into(), user.into()],
    )
    .await
    .unwrap();
    let task = f.queued_analysis("proposal-tools", true).await;
    let bound = bind(&f, task, "proposal-tools").await;
    let producer = actor(
        PRODUCER,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    let storage = domain::InMemoryStorage::default();
    let issue = shared::IssueId::from_uuid(task);
    let bytes = b"# Analysis\nSynthetic source and specification for proposal transport.";
    let attachment = artifacts::publish(
        &f.db,
        &f.repo.config,
        &producer,
        issue,
        ArtifactUpload {
            file_name: "analysis.md".into(),
            content_type: "text/markdown".into(),
            bytes: bytes.to_vec(),
        },
        &storage,
    )
    .await
    .unwrap();
    let duplicate = artifacts::publish(
        &f.db,
        &f.repo.config,
        &producer,
        issue,
        ArtifactUpload {
            file_name: "analysis.md".into(),
            content_type: "text/markdown".into(),
            bytes: bytes.to_vec(),
        },
        &storage,
    )
    .await
    .unwrap();
    assert_eq!(attachment.id, duplicate.id);
    assert_eq!(
        artifacts::download(&f.db, &f.repo.config, &producer, attachment.id, &storage)
            .await
            .unwrap()
            .1,
        bytes
    );
    let seal = Seal::parse(attachment.storage_key.as_ref())
        .unwrap()
        .unwrap();
    let command = SubmitDeliveryProposal {
        fence: delivery_fence(&bound),
        workflow_receipt: ArtifactReceipt {
            reference: "fixture://synthetic-workflow-terminal".into(),
            sha256: "d".repeat(64),
        },
        result: DeliveryResult::Analyst {
            analysis: seal.artifact().receipt,
        },
        idempotency_key: "tools:proposal".into(),
    };
    assert!(
        f.repo
            .submit_delivery_proposal(task, &producer, command.clone())
            .await
            .is_err(),
        "final ordinary comment is required"
    );
    let assignment = bound.assignment.as_ref().unwrap();
    let comment = PublishStageComment {
        fence: MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            assignment_version: assignment.fencing_token,
        },
        body: format!(
            "QA analysis: attachment {}. Synthetic Workflow receipt only; no stage handoff.",
            attachment.id
        ),
        idempotency_key: "tools:comment".into(),
    };
    let (saved, replay) = f
        .repo
        .publish_stage_comment(task, &producer, comment.clone())
        .await
        .unwrap();
    assert!(!replay);
    assert!(
        f.repo
            .publish_stage_comment(task, &producer, comment)
            .await
            .unwrap()
            .1
    );
    let mut wrong_binding = command.clone();
    wrong_binding.fence.execution_id = Uuid::new_v4();
    wrong_binding.idempotency_key = "tools:wrong-binding".into();
    assert!(
        f.repo
            .submit_delivery_proposal(task, &producer, wrong_binding)
            .await
            .is_err()
    );
    exec(&f.db,"UPDATE comments SET body='Edited ordinary report',updated_at=clock_timestamp() WHERE id=$1",vec![saved.comment_id.into()]).await.unwrap();
    assert!(
        f.repo
            .stage_comment_operation(task, &producer, "tools:comment")
            .await
            .is_err()
    );
    assert!(
        f.repo
            .submit_delivery_proposal(task, &producer, command.clone())
            .await
            .is_err(),
        "edited comment cannot authorize a proposal"
    );
    crate::sdlc::configured_test_support::delivery_comment(
        &f.repo,
        task,
        &producer,
        assignment,
        "tools:replacement-comment",
    )
    .await;
    let (proposal, replay) = f
        .repo
        .submit_delivery_proposal(task, &producer, command.clone())
        .await
        .unwrap();
    assert!(!replay);
    let (same, replay) = f
        .repo
        .submit_delivery_proposal(task, &producer, command.clone())
        .await
        .unwrap();
    assert!(replay);
    assert_eq!(
        serde_json::to_value(same).unwrap(),
        serde_json::to_value(&proposal).unwrap()
    );
    let mut changed = command;
    let DeliveryResult::Analyst { analysis } = &mut changed.result else {
        unreachable!()
    };
    analysis.sha256 = "e".repeat(64);
    assert!(
        f.repo
            .submit_delivery_proposal(task, &producer, changed)
            .await
            .is_err(),
        "replay cannot replace the artifact hash"
    );
    let current = f
        .repo
        .lifecycle_read(task, &f.reader)
        .await
        .unwrap()
        .receipt;
    assert_eq!(current.task.stage, LifecycleStage::Analysis);
    assert_eq!(current.task.status, LifecycleStatus::InProgress);
    assert_eq!(
        current.assignment.as_ref().unwrap().assignment_id,
        assignment.assignment_id
    );
    assert_eq!(
        serde_json::to_value(current.result).unwrap(),
        serde_json::to_value(bound.result).unwrap()
    );
}

async fn bind(f: &Fixture, task: Uuid, key: &str) -> LifecycleReceipt {
    let receipt = f
        .repo
        .lifecycle_read(task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let subject = match receipt.task.stage {
        LifecycleStage::Analysis => PRODUCER,
        LifecycleStage::Architecture => CONSUMER,
        _ => panic!("this artifact fixture models Analyst and Architect only"),
    };
    let (reserved, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Reserve {
                expected_task_version: receipt.task.version,
                machine_subject: Some(subject.into()),
                idempotency_key: format!("reserve-{key}"),
            },
        )
        .await
        .unwrap();
    let (bound, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&reserved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: subject.into(),
                    dispatch_receipt: ArtifactReceipt {
                        reference: "fixture://synthetic-fleet-dispatch".into(),
                        sha256: "c".repeat(64),
                    },
                },
                idempotency_key: format!("bind-{key}"),
            },
        )
        .await
        .unwrap();
    bound
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; synthetic runtime observations"]
async fn actual_role_artifacts_are_fenced_replayed_predecessor_scoped_and_immutable() {
    let f = Fixture::new().await;
    let mut service_users = std::collections::HashMap::new();
    for (index, subject) in [PRODUCER, CONSUMER].into_iter().enumerate() {
        let user_id = Uuid::new_v4();
        service_users.insert(subject, user_id);
        exec(
            &f.db,
            "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
            VALUES($1,$2,$3,'Artifact fixture service','!',$3,true)",
            vec![
                user_id.into(),
                format!("artifact-{index}@example.test").into(),
                subject.into(),
            ],
        )
        .await
        .unwrap();
        exec(
            &f.db,
            "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
            vec![f.project.into(), user_id.into()],
        )
        .await
        .unwrap();
    }
    let task = f.queued_analysis("artifact-task", true).await;
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::EnsureQueued {
                idempotency_key: "queue".into(),
            },
        )
        .await
        .unwrap();
    let bound = bind(&f, task, "analyst").await;
    assert_eq!(bound.lease.as_ref().unwrap().holder_subject, "scheduler");
    let assignment = bound.assignment.as_ref().unwrap();
    let producer = actor(
        PRODUCER,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    let storage = domain::InMemoryStorage::default();
    let issue = shared::IssueId::from_uuid(task);
    let config = &f.repo.config;
    let input_id = Uuid::new_v4();
    let input_key = "ordinary-research-input.txt";
    let input_bytes = b"Owner-provided business source";
    domain::FileStorage::put(&storage, &task.to_string(), input_key, input_bytes.to_vec())
        .await
        .unwrap();
    exec(&f.db,
        "INSERT INTO attachments(id,issue_id,author_id,file_name,content_type,size_bytes,storage_key,created_at)
         SELECT $1,$2,id,'business-input.txt','text/plain',$3,$4,now() FROM users WHERE central_sub='owner'",
        vec![input_id.into(),task.into(),(input_bytes.len() as i64).into(),input_key.into()],
    ).await.unwrap();
    let input_id = shared::AttachmentId::from_uuid(input_id);
    assert_eq!(
        artifacts::download(&f.db, config, &producer, input_id, &storage)
            .await
            .unwrap()
            .1,
        input_bytes
    );
    assert_eq!(
        artifacts::list(&f.db, config, &producer, issue)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        artifacts::download(&f.db, config, &f.reader, input_id, &storage)
            .await
            .is_err()
    );
    let unrelated = actor(
        CONSUMER,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &unrelated, input_id, &storage).await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "UPDATE attachments SET size_bytes=size_bytes+1 WHERE id=$1",
        vec![input_id.as_uuid().into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        artifacts::download(&f.db, config, &producer, input_id, &storage).await,
        Err(AppError::Conflict(_))
    ));
    exec(
        &f.db,
        "UPDATE attachments SET size_bytes=size_bytes-1 WHERE id=$1",
        vec![input_id.as_uuid().into()],
    )
    .await
    .unwrap();
    // Losing native project membership immediately removes ordinary input access.
    exec(
        &f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![f.project.into(), service_users[PRODUCER].into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        artifacts::download(&f.db, config, &producer, input_id, &storage).await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![f.project.into(), service_users[PRODUCER].into()],
    )
    .await
    .unwrap();
    let bytes = b"{ \"actual\": \"analysis\" }";
    let (one, two) = tokio::join!(
        artifacts::publish(
            &f.db,
            config,
            &producer,
            issue,
            upload(bytes, "analysis.json"),
            &storage
        ),
        artifacts::publish(
            &f.db,
            config,
            &producer,
            issue,
            upload(bytes, "analysis.json"),
            &storage
        )
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.id, two.id);
    assert_eq!(one.created_at, two.created_at);
    assert_eq!(one.author_id.as_uuid(), service_users[PRODUCER]);
    let seal = Seal::parse(one.storage_key.as_ref()).unwrap().unwrap();
    assert_eq!(seal.run_id, bound.binding.as_ref().unwrap().run_id);
    let (_, actual) = artifacts::download(&f.db, config, &producer, one.id, &storage)
        .await
        .unwrap();
    assert_eq!(actual, bytes);
    assert!(matches!(
        artifacts::download(&f.db, config, &f.reader, one.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let foreign = actor(
        CONSUMER,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &foreign, one.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let owner_impersonation = actor("owner", false, &["task-tracker:read", "task-tracker:write"]);
    assert!(matches!(
        artifacts::publish(
            &f.db,
            config,
            &owner_impersonation,
            issue,
            upload(bytes, "owner.json"),
            &storage
        )
        .await,
        Err(AppError::Forbidden)
    ));
    let retired_grant = actor(
        PRODUCER,
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &format!(
                "task-tracker:sdlc:delivery:{}:{}:{}:{}:{}",
                task,
                assignment.assignment_id,
                assignment.execution_id,
                assignment.agent_id,
                assignment.fencing_token + 1
            ),
        ],
    );
    assert!(matches!(
        artifacts::publish(
            &f.db,
            config,
            &retired_grant,
            issue,
            upload(bytes, "retired-grant.json"),
            &storage
        )
        .await,
        Err(AppError::Forbidden)
    ));
    let original = one.storage_key.to_string();
    exec(
        &f.db,
        "UPDATE attachments SET size_bytes=size_bytes+1 WHERE id=$1",
        vec![one.id.as_uuid().into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        artifacts::download(&f.db, config, &producer, one.id, &storage).await,
        Err(AppError::Conflict(_))
    ));
    exec(
        &f.db,
        "UPDATE attachments SET size_bytes=size_bytes-1 WHERE id=$1",
        vec![one.id.as_uuid().into()],
    )
    .await
    .unwrap();
    assert!(
        domain::FileStorage::delete(&storage, &task.to_string(), &original)
            .await
            .is_err()
    );
    let omitted = artifacts::publish(
        &f.db,
        config,
        &producer,
        issue,
        upload(b"{}", "unaccepted.json"),
        &storage,
    )
    .await
    .unwrap();
    let result = DeliveryResult::Analyst {
        analysis: seal.artifact().receipt,
    };
    let workflow_receipt = ArtifactReceipt {
        reference: "fixture://synthetic-workflow-terminal".into(),
        sha256: "d".repeat(64),
    };
    crate::sdlc::configured_test_support::delivery_comment(
        &f.repo,
        task,
        &producer,
        bound.assignment.as_ref().unwrap(),
        "artifact:final-comment",
    )
    .await;
    let (proposal, _) = f
        .repo
        .submit_delivery_proposal(
            task,
            &producer,
            SubmitDeliveryProposal {
                fence: delivery_fence(&bound),
                workflow_receipt: workflow_receipt.clone(),
                result: result.clone(),
                idempotency_key: "proposal".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        artifacts::download(&f.db, config, &f.reader, one.id, &storage)
            .await
            .unwrap()
            .1,
        bytes
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &f.reader, omitted.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let binding = bound.binding.as_ref().unwrap();
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Complete {
                fence: delivery_fence(&bound),
                result,
                terminal: TerminalReceipt {
                    chat_id: binding.chat_id,
                    run_id: binding.run_id,
                    workflow_receipt,
                    fleet_receipt: ArtifactReceipt {
                        reference: "fixture://synthetic-native-terminal".into(),
                        sha256: "e".repeat(64),
                    },
                    output_sha256: proposal.output_sha256,
                },
                idempotency_key: "complete".into(),
            },
        )
        .await
        .unwrap();
    let architect = bind(&f, task, "architect").await;
    assert_eq!(
        architect
            .assignment
            .as_ref()
            .unwrap()
            .machine_subject
            .as_deref(),
        Some(CONSUMER)
    );
    let consumer = actor(
        CONSUMER,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &consumer, input_id, &storage).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        artifacts::download(&f.db, config, &producer, input_id, &storage).await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(
        artifacts::download(&f.db, config, &consumer, one.id, &storage)
            .await
            .unwrap()
            .1,
        bytes
    );
    assert!(matches!(
        artifacts::download(&f.db, config, &consumer, omitted.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(
        artifacts::list(&f.db, config, &consumer, issue)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        artifacts::publish(
            &f.db,
            config,
            &producer,
            issue,
            upload(bytes, "analysis.json"),
            &storage
        )
        .await
        .is_err()
    );
    // Actual database clock, no trigger disabling or protected ledger edits.
    f.db.execute_unprepared("SELECT pg_sleep(31)")
        .await
        .unwrap();
    assert!(matches!(
        artifacts::publish(
            &f.db,
            config,
            &consumer,
            issue,
            upload(b"{}", "expired.json"),
            &storage
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    exec(
        &f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![f.project.into(), service_users[CONSUMER].into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        artifacts::download(&f.db, config, &consumer, one.id, &storage).await,
        Err(AppError::Forbidden)
    ));
    let owner_id: Uuid =
        f.db.query_one(statement(
            "SELECT id FROM users WHERE central_sub='owner'",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    let foreign_id: Uuid =
        f.db.query_one(statement(
            "SELECT id FROM users WHERE central_sub='foreign'",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    // Transfer the existing fixture connection, never clone it or create another pool.
    let attachments = crate::repos::SeaOrmRepositories::new(f.db).attachments;
    attachments
        .authorize_sealed_human_read(issue, shared::UserId::from_uuid(owner_id))
        .await
        .unwrap();
    assert!(matches!(
        attachments
            .authorize_sealed_human_read(issue, shared::UserId::from_uuid(foreign_id))
            .await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        attachments.delete(one.id).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(attachments.get_by_id(one.id).await.unwrap().id, one.id);
}
