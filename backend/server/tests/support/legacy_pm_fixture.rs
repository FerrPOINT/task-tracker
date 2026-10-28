//! Stored predecessor snapshots for legacy read/answer acceptance, not live PM writers.
use super::*;
use sea_orm::TransactionTrait;

async fn legacy_state(tx: &sea_orm::DatabaseTransaction, task: Uuid) -> TaskState {
    let name: String = tx
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS name".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert!(name.ends_with("_sdlc_test"));
    let row = tx.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT state,pm_execution_id IS NULL AS unreserved FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",[task.into()]))
        .await.unwrap().unwrap();
    assert!(row.try_get::<bool>("", "unreserved").unwrap());
    let state: TaskState = serde_json::from_value(row.try_get("", "state").unwrap()).unwrap();
    assert!(!state.configured_cli());
    assert!(matches!(state.stage, Stage::Draft | Stage::Clarification));
    state
}

async fn save(tx: &sea_orm::DatabaseTransaction, state: &TaskState, kind: &str, result: &Value) {
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
        [state.task_id.into(), json!(state).into()],
    ))
    .await
    .unwrap();
    let event = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,
        "task_id":state.task_id,"root_task_id":state.root_task_id,"owner_subject":state.owner_subject,"stage":state.stage,
        "requirement_revision":state.current_revision(),"result":result});
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
        [
            Uuid::new_v4().into(),
            state.task_id.into(),
            kind.into(),
            event.into(),
        ],
    ))
    .await
    .unwrap();
}

pub async fn revision(
    db: &DatabaseConnection,
    client: &Client,
    url: &str,
    task: Uuid,
    input: &Value,
) -> Value {
    post(
        client,
        &format!("{url}/requirements"),
        "sdlc_pat_pm",
        input,
        403,
    )
    .await;
    let command: PublishRevision = serde_json::from_value(input.clone()).unwrap();
    let tx = db.begin().await.unwrap();
    let mut state = legacy_state(&tx, task).await;
    assert_eq!(
        state.current_revision(),
        command.expected_requirement_revision
    );
    assert!(
        !state
            .questions
            .iter()
            .any(|question| question.required && question.state == QuestionState::Open)
    );
    app::sdlc::validate_document(&command.document).unwrap();
    let revision = RequirementsRevision {
        revision: command.expected_requirement_revision.unwrap_or(0) + 1,
        content_hash: app::sdlc::canonical_hash(&command.document).unwrap(),
        document: command.document,
        author_subject: "pm".into(),
        created_at: shared::now().to_utc(),
    };
    let result = json!(revision);
    state.confirmation_revision = Some(revision.revision);
    state.revisions.push(revision.clone());
    for question in &mut state.questions {
        if question.state == QuestionState::Open {
            question.state = QuestionState::Superseded;
        }
    }
    if state.draft_submode.is_some() {
        state.stage = Stage::Draft;
        state.draft_submode = Some(DraftSubmode::AwaitingConfirmation);
    } else {
        state.stage = Stage::Clarification;
    }
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO sdlc_requirements(task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4)",
        [
            task.into(),
            revision.revision.into(),
            revision.content_hash.into(),
            result.clone().into(),
        ],
    ))
    .await
    .unwrap();
    save(&tx, &state, "requirements.published", &result).await;
    tx.commit().await.unwrap();
    result
}

pub async fn cancel(db: &DatabaseConnection, task: Uuid, question: Uuid) {
    let tx = db.begin().await.unwrap();
    let mut state = legacy_state(&tx, task).await;
    let saved = state
        .questions
        .iter_mut()
        .find(|saved| saved.id == question)
        .unwrap();
    assert_eq!(saved.state, QuestionState::Open);
    saved.state = QuestionState::Cancelled;
    let result = json!(saved);
    save(&tx, &state, "clarification.cancelled", &result).await;
    tx.commit().await.unwrap();
}

pub async fn import(
    db: &DatabaseConnection,
    client: &Client,
    url: &str,
    task: Uuid,
    input: &Value,
) -> Value {
    // New legacy publications stay rejected. Current configured publications
    // have their own HTTP scenario in configured_pm_http.rs.
    post(
        client,
        &format!("{url}/clarifications"),
        "sdlc_pat_pm",
        input,
        403,
    )
    .await;
    let command: PublishQuestion = serde_json::from_value(input.clone()).unwrap();
    let tx = db.begin().await.unwrap();
    let mut state = legacy_state(&tx, task).await;
    assert_eq!(state.current_revision(), Some(command.requirement_revision));
    let previous = state
        .questions
        .iter()
        .position(|question| question.id == command.question_id);
    assert_eq!(
        previous.map(|index| state.questions[index].version),
        command.expected_question_version
    );
    let checkpoint = previous
        .and_then(|index| state.questions[index].checkpoint_id)
        .unwrap_or_else(Uuid::new_v4);
    let question = Question {
        id: command.question_id,
        request_id: command.request_id,
        task_id: task,
        root_task_id: state.root_task_id,
        version: command.expected_question_version.unwrap_or(0) + 1,
        requirement_revision: command.requirement_revision,
        requirement_reference: command.requirement_reference,
        assignment_id: command.fence.assignment_id,
        execution_id: command.fence.execution_id,
        agent_id: command.fence.agent_id,
        assignment_version: command.fence.assignment_version,
        checkpoint_id: Some(checkpoint),
        text: command.text,
        rationale: command.rationale,
        required: command.required,
        mode: command.mode,
        options: command.options,
        recommended_option_id: command.recommended_option_id,
        state: QuestionState::Open,
        answer: None,
        author_subject: "pm".into(),
        created_at: shared::now().to_utc(),
    };
    if let Some(index) = previous {
        state.questions[index] = question.clone();
    } else {
        state.questions.push(question.clone());
    }
    state.confirmation_revision = None;
    if state.draft_submode.is_some() {
        state.stage = Stage::Draft;
        state.draft_submode = Some(DraftSubmode::Clarification);
    } else {
        state.stage = Stage::Clarification;
    }
    let result = json!(question);
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_requests(request_id,task_id) VALUES($1,$2) ON CONFLICT(request_id) DO NOTHING",
        [question.request_id.into(),task.into()])).await.unwrap();
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_question_versions(task_id,question_id,version,request_id,payload) VALUES($1,$2,$3,$4,$5)",
        [task.into(),question.id.into(),question.version.into(),question.request_id.into(),result.clone().into()])).await.unwrap();
    for option in &question.options {
        tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
            "INSERT INTO sdlc_options(task_id,question_id,question_version,option_id,payload) VALUES($1,$2,$3,$4,$5)",
            [task.into(),question.id.into(),question.version.into(),option.id.into(),json!(option).into()])).await.unwrap();
    }
    save(&tx, &state, "clarification.published", &result).await;
    tx.commit().await.unwrap();
    result
}
