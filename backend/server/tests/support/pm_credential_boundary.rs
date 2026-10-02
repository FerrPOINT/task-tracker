use super::*;

pub fn tokens(
    tokens: &mut HashMap<String, Value>,
    assignment: &PmAssignment,
    replacement: &PmAssignment,
    task: Uuid,
) {
    for (name, subject, grants) in [
        ("sdlc_pat_pm_generic", "pm", vec![]),
        (
            "sdlc_pat_pm_foreign_subject",
            "foreign",
            vec![assignment.scope(task)],
        ),
        (
            "sdlc_pat_pm_unknown_subject",
            "not-linked-pm",
            vec![assignment.scope(task)],
        ),
        (
            "sdlc_pat_pm_ambiguous",
            "pm",
            vec![assignment.scope(task), replacement.scope(task)],
        ),
        (
            "sdlc_pat_pm_malformed",
            "pm",
            vec!["task-tracker:sdlc:pm:invalid".into()],
        ),
        (
            "sdlc_pat_pm_noncanonical",
            "pm",
            vec![assignment.scope(task).trim_end_matches('1').to_owned() + "01"],
        ),
    ] {
        let mut scopes = vec![
            "task-tracker:read".to_string(),
            "task-tracker:write".to_string(),
        ];
        scopes.extend(grants);
        tokens.insert(
            name.into(),
            json!({"sub":subject,"email":format!("{subject}@example.test"),"scopes":scopes}),
        );
    }
}

async fn status(
    client: &Client,
    base: &str,
    method: reqwest::Method,
    path: &str,
    token: &str,
    expected: u16,
) {
    let response = client
        .request(method, format!("{base}/api/v1/{path}"))
        .bearer_auth(token)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    let code = response.status().as_u16();
    let body = response.text().await.unwrap();
    assert_eq!(code, expected, "{path}: {body}");
    if expected == 403 || expected == 503 {
        assert!(
            !body.contains("Clarify")
                && !body.contains("private-result")
                && !body.contains("Approved goal"),
            "denial disclosed content"
        );
    }
}

pub async fn initial(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    task: Uuid,
    foreign: Uuid,
    project: Uuid,
    owner: &str,
) {
    let before = [count(db, "users").await, count(db, "sdlc_outbox").await];
    let prefix = format!("issues/{task}/sdlc");
    for target in [task, foreign] {
        for method in [
            reqwest::Method::GET,
            reqwest::Method::PATCH,
            reqwest::Method::DELETE,
        ] {
            status(
                client,
                base,
                method,
                &format!("issues/{target}"),
                "sdlc_pat_pm",
                403,
            )
            .await;
        }
    }
    for path in [
        "projects",
        "users",
        "issues",
        "statuses",
        "sdlc/project-access",
        "sdlc/project-directory",
    ] {
        status(client, base, reqwest::Method::GET, path, "sdlc_pat_pm", 403).await;
    }
    for path in [
        format!("projects/{project}/sdlc/drafts"),
        format!("{prefix}/binding"),
        format!("{prefix}/assignment"),
        format!("{prefix}/pm-draft-assignment"),
        format!("{prefix}/evidence"),
        format!("{prefix}/requirements/1/confirm"),
        format!("{prefix}/clarifications/{}/answers", Uuid::new_v4()),
    ] {
        status(
            client,
            base,
            reqwest::Method::POST,
            &path,
            "sdlc_pat_pm",
            403,
        )
        .await;
    }
    for path in [
        format!("projects/{project}/sdlc/drafts/operations/unknown"),
        format!("{prefix}/pm-draft-assignment"),
        format!("issues/{foreign}/sdlc/context"),
        format!("issues/{foreign}/sdlc/events?projection=metadata_v1"),
        format!("issues/{}/sdlc/context", Uuid::new_v4()),
    ] {
        status(
            client,
            base,
            reqwest::Method::GET,
            &path,
            "sdlc_pat_pm",
            403,
        )
        .await;
    }
    status(
        client,
        base,
        reqwest::Method::HEAD,
        &format!("{prefix}/context"),
        "sdlc_pat_pm",
        403,
    )
    .await;
    for token in [
        "sdlc_pat_pm_foreign_subject",
        "sdlc_pat_pm_unknown_subject",
        "sdlc_pat_pm_ambiguous",
        "sdlc_pat_pm_malformed",
        "sdlc_pat_pm_noncanonical",
        "sdlc_pat_replacement",
    ] {
        status(
            client,
            base,
            reqwest::Method::GET,
            &format!("{prefix}/context"),
            token,
            403,
        )
        .await;
        status(
            client,
            base,
            reqwest::Method::GET,
            &format!("issues/{foreign}"),
            token,
            403,
        )
        .await;
    }
    for resource in [
        "context",
        "clarifications",
        "requirements/revisions",
        "events",
        "events?projection=metadata_v1",
    ] {
        status(
            client,
            base,
            reqwest::Method::GET,
            &format!("{prefix}/{resource}"),
            "sdlc_pat_pm",
            200,
        )
        .await;
    }
    for token in ["sdlc_pat_pm_generic", owner] {
        status(
            client,
            base,
            reqwest::Method::GET,
            &format!("issues/{foreign}"),
            token,
            200,
        )
        .await;
    }
    sql(db, "DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='pm')", vec![project.into()]).await;
    for resource in [
        "context",
        "clarifications",
        "requirements/revisions",
        "events?projection=metadata_v1",
    ] {
        status(
            client,
            base,
            reqwest::Method::GET,
            &format!("{prefix}/{resource}"),
            "sdlc_pat_pm",
            403,
        )
        .await;
    }
    sql(db, "INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='pm'", vec![project.into()]).await;
    assert_eq!(
        before,
        [count(db, "users").await, count(db, "sdlc_outbox").await]
    );
    println!(
        "PM_BOUNDARY initial opaque PAT legacy/global/task/subject/grant/ACL/no-side-effects checks passed"
    );
}

pub async fn current(client: &Client, base: &str, task: Uuid, replaced: bool) {
    for resource in [
        "context",
        "clarifications",
        "requirements",
        "requirements/revisions",
        "requirements/1",
        "requirements/1/diff?against=1",
        "events",
        "events?projection=metadata_v1",
    ] {
        let path = format!("issues/{task}/sdlc/{resource}");
        status(
            client,
            base,
            reqwest::Method::GET,
            &path,
            "sdlc_pat_pm",
            if replaced { 403 } else { 200 },
        )
        .await;
        if replaced {
            status(
                client,
                base,
                reqwest::Method::GET,
                &path,
                "sdlc_pat_replacement",
                200,
            )
            .await;
        }
    }
    println!("PM_BOUNDARY current reads/restart/replacement replaced={replaced} passed");
}

pub async fn unavailable(client: &Client, base: &str, task: Uuid) {
    for path in [
        format!("issues/{task}"),
        format!("issues/{task}/sdlc/context"),
    ] {
        status(
            client,
            base,
            reqwest::Method::GET,
            &path,
            "sdlc_pat_replacement",
            503,
        )
        .await;
    }
    println!("PM_BOUNDARY fresh Auth outage denies legacy and strict without disclosure");
}
