//! Controlled PostgreSQL intake; no Fleet, model or research agent runs here.
use super::*;
use domain::background_intake::*;

fn finding(destination: FindingDestination) -> FindingIntake {
    FindingIntake {
        source: FindingOrigin {
            application: FindingApplication::FleetControl,
            finding_id: Uuid::new_v4(),
            finding_revision: 1,
        },
        destination,
        kind: FindingKind::Bug,
        title: "Observed contract violation".into(),
        description: "Exact reviewed description".into(),
        expected: "Exact reviewed expectation".into(),
        criteria: "Exact reviewed acceptance".into(),
        evidence: "repo/file.rs:12 at explicit fixture SHA".into(),
        custom_fields: Default::default(),
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn background_finding_human_intake_required_fields_replay_and_real_analysis_queue() {
    Box::pin(async {
        let f=Box::pin(Fixture::new()).await;
        let field=Uuid::new_v4();
        exec(&f.db,"INSERT INTO custom_fields(id,project_id,name,field_type,is_required) VALUES($1,$2,'Required decision','text',true)",
            vec![field.into(),f.project.into()]).await.unwrap();
        let operation=Uuid::new_v4();
        let mut draft=finding(FindingDestination::Draft);
        let before=f.snapshot().await;
        assert!(matches!(f.repo.intake_finding(f.project,&f.owner,operation,draft.clone()).await,Err(AppError::Validation(_))));
        assert_eq!(f.snapshot().await,before);
        draft.custom_fields.insert(field.to_string(),json!("Human selected"));
        assert!(matches!(f.repo.intake_finding(f.project,&f.reader,operation,draft.clone()).await,Err(AppError::Forbidden)));
        let a=f.repo.intake_finding(f.project,&f.owner,operation,draft.clone());
        let b=f.repo.intake_finding(f.project,&f.owner,operation,draft.clone());
        let (a,b)=tokio::join!(a,b);let (a,ar)=a.unwrap();let (b,br)=b.unwrap();
        assert_eq!(a,b);assert_ne!(ar,br);
        assert_eq!(f.repo.finding_intake_operation(f.project,&f.owner,operation).await.unwrap(),a);
        let row=f.db.query_one(statement("SELECT issue_type,(SELECT value FROM issue_custom_field_values WHERE issue_id=i.id AND field_id=$2) AS value FROM issues i WHERE i.id=$1",
            vec![a.task_id.into(),field.into()])).await.unwrap().unwrap();
        assert_eq!(row.try_get::<String>("","issue_type").unwrap(),"bug");
        assert_eq!(row.try_get::<Json>("","value").unwrap(),json!("Human selected"));
        let state=f.repo.read(a.task_id,&f.owner).await.unwrap();
        assert_eq!(state.stage,Stage::Draft);assert!(state.assignment.is_none());
        assert!(!state.retained_fields.contains_key("lifecycle"));
        let before=f.snapshot().await;
        draft.title="Changed after lost reply".into();
        assert!(matches!(f.repo.intake_finding(f.project,&f.owner,operation,draft).await,Err(AppError::Conflict(_))));
        assert_eq!(f.snapshot().await,before);
        let mut backlog=finding(FindingDestination::Backlog);backlog.kind=FindingKind::Refactoring;
        backlog.custom_fields.insert(field.to_string(),json!("Reviewed for analysis"));
        let operation=Uuid::new_v4();
        let (receipt,replay)=f.repo.intake_finding(f.project,&f.owner,operation,backlog.clone()).await.unwrap();assert!(!replay);
        assert_eq!(receipt.destination,FindingDestination::Backlog);
        let state=f.repo.read(receipt.task_id,&f.owner).await.unwrap();
        assert_eq!(state.stage,Stage::Analysis);assert!(state.assignment.is_none());
        assert!(state.evidence.is_empty());
        app::sdlc::validate_document(&state.revisions[0].document).unwrap();
        assert_eq!(state.revisions[0].document.dependencies,vec![backlog.evidence.clone()]);
        assert_eq!(state.revisions[0].document.scope,vec![backlog.description]);
        let delivery=f.repo.lifecycle_read(receipt.task_id,&f.reader).await.unwrap().receipt;
        assert_eq!(delivery.task.runtime,RuntimeState::Queued);
        assert_eq!(delivery.task.stage,LifecycleStage::Analysis);
        let intent=f.repo.analysis_intent(receipt.task_id,&f.owner).await.unwrap();
        assert_eq!(intent.confirmation_id,delivery.task.routing.confirmation_id);
        let row=f.db.query_one(statement("SELECT issue_type,(SELECT count(*) FROM sdlc_pm_executions WHERE task_id=i.id) AS pm,
            (SELECT count(*) FROM sdlc_configured_pm_completions WHERE task_id=i.id) AS completions FROM issues i WHERE i.id=$1",
            vec![receipt.task_id.into()])).await.unwrap().unwrap();
        assert_eq!(row.try_get::<String>("","issue_type").unwrap(),"task");
        assert_eq!(row.try_get::<i64>("","pm").unwrap(),0);assert_eq!(row.try_get::<i64>("","completions").unwrap(),0);
        let page=f.repo.read_delivery_candidates(&f.reader,f.query(None,100)).await.unwrap();
        assert_eq!(page.items.len(),1);assert_eq!(page.items[0].task_id,receipt.task_id);
        let states=domain::sdlc_source::TaskSourceRepository::background_task_states(&f.repo,f.project,&[a.task_id,receipt.task_id]).await.unwrap();
        assert_eq!(states.tasks.len(),2);assert!(states.tasks.iter().all(|s|!s.completed));
        assert!(domain::sdlc_source::TaskSourceRepository::background_task_states(&f.repo,Uuid::new_v4(),&[receipt.task_id]).await.is_err());
        let before=f.snapshot().await;
        assert_eq!(f.repo.finding_intake_operation(f.project,&f.owner,operation).await.unwrap(),receipt);
        assert_eq!(f.snapshot().await,before);
    }).await;
}
