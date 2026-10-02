use super::*;

pub struct Fixture<'a> {
    pub db: &'a DatabaseConnection,
    pub client: &'a Client,
    pub base: &'a str,
    pub owner: &'a str,
    pub operator: &'a str,
    pub foreign: &'a str,
    pub owner_id: Uuid,
    pub operator_id: Uuid,
    pub unavailable: &'a AtomicBool,
}

fn assert_page(page: &Value, limit: usize) -> Vec<Value> {
    assert_eq!(page.as_object().unwrap().len(), 4);
    assert_eq!(page["contract_version"], 1);
    assert_eq!(page["tracker_instance_id"], "tracker-draft-test");
    let projects = page["projects"].as_array().unwrap();
    assert!(projects.len() <= limit);
    let mut previous = None;
    for project in projects {
        assert_eq!(project.as_object().unwrap().len(), 3);
        let raw = project["id"].as_str().unwrap();
        let id = Uuid::parse_str(raw).unwrap();
        assert!(!id.is_nil());
        assert_eq!(id.to_string(), raw);
        assert!(previous.is_none_or(|before| before < id));
        previous = Some(id);
        assert!(project["key"].is_string());
        assert!(project["name"].is_string());
    }
    let cursor = page.get("next_cursor").expect("required even when null");
    if !cursor.is_null() {
        assert_eq!(projects.len(), limit);
        assert_eq!(cursor, &projects.last().unwrap()["id"]);
    }
    projects.clone()
}

async fn curl_get(url: &str, token: Option<&str>) -> (u16, Value) {
    let (url, token) = (url.to_owned(), token.map(str::to_owned));
    let output = tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new("curl");
        command.args([
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "--write-out",
            "\n%{http_code}",
        ]);
        if let Some(token) = token {
            command
                .arg("--header")
                .arg(format!("Authorization: Bearer {token}"));
        }
        command.arg("--url").arg(url).output().unwrap()
    })
    .await
    .unwrap();
    assert!(output.status.success(), "directory curl transport failed");
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let (body, status) = stdout.rsplit_once('\n').unwrap();
    (status.parse().unwrap(), serde_json::from_str(body).unwrap())
}

pub async fn verify(f: Fixture<'_>) {
    let url = format!("{}/api/v1/sdlc/project-directory", f.base);
    let initial = get_json(f.client, &url, f.owner, 200).await;
    let mut expected = assert_page(&initial, 50);
    assert_eq!(expected.len(), 2);
    assert_eq!(initial["next_cursor"], Value::Null);
    for token in [f.owner, "sdlc_pat_read"] {
        let (status, page) = curl_get(&url, Some(token)).await;
        assert_eq!(status, 200);
        assert_eq!(assert_page(&page, 50), expected);
        assert_eq!(page, initial);
    }
    for (token, expected_status) in [(None, 401), (Some("sdlc_pat_other"), 403)] {
        let (status, denial) = curl_get(&url, token).await;
        assert_eq!(status, expected_status);
        let denial = denial.to_string();
        for project in &expected {
            assert!(!denial.contains(project["id"].as_str().unwrap()));
        }
        assert!(!denial.contains("projects") && !denial.contains("next_cursor"));
    }
    println!(
        "PROJECT_DIRECTORY_CURL human/PAT 200 exact page, unauthenticated 401 and wrong-scope 403 without directory disclosure passed"
    );
    // Operator owns both projects and is also a member of one: no duplicate rows.
    assert_eq!(get_json(f.client, &url, f.operator, 200).await, initial);
    assert_eq!(
        get_json(f.client, &url, "sdlc_pat_read", 200).await,
        initial
    );
    let empty = get_json(f.client, &url, f.foreign, 200).await;
    assert!(assert_page(&empty, 50).is_empty());
    assert_eq!(empty["next_cursor"], Value::Null);

    let mut own = Vec::new();
    for n in 0..103 {
        let id = Uuid::new_v4();
        let key = format!("DIR{n}");
        let name = format!("Directory {n} \"\u{0416}\"\\");
        sql(f.db, "INSERT INTO projects(id,key,name,description,owner_id,default_board_id) VALUES($1,$2,$3,'directory-private-marker',$4,$5)",
            vec![id.into(),key.clone().into(),name.clone().into(),f.owner_id.into(),Uuid::new_v4().into()]).await;
        sql(f.db, "INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Directory test','[]' FROM projects WHERE id=$1", vec![id.into()]).await;
        own.push(id);
        expected.push(json!({"id":id,"key":key,"name":name}));
    }
    sql(
        f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![own[0].into(), f.owner_id.into()],
    )
    .await;
    expected.sort_by_key(|p| Uuid::parse_str(p["id"].as_str().unwrap()).unwrap());

    for (query, limit) in [("", 50), ("?limit=1", 1), ("?limit=100", 100)] {
        let page = get_json(f.client, &format!("{url}{query}"), f.owner, 200).await;
        assert_eq!(assert_page(&page, limit), expected[..limit]);
        assert_eq!(page["next_cursor"], expected[limit - 1]["id"]);
        assert!(!page.to_string().contains("directory-private-marker"));
    }
    let mut collected = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let query = cursor
            .as_ref()
            .map(|id| format!("?limit=7&after={id}"))
            .unwrap_or_else(|| "?limit=7".into());
        let page = get_json(f.client, &format!("{url}{query}"), f.owner, 200).await;
        let rows = assert_page(&page, 7);
        assert!(!rows.is_empty());
        if let Some(ref after) = cursor {
            assert!(rows[0]["id"].as_str().unwrap() > after.as_str());
        }
        collected.extend(rows);
        assert!(collected.len() <= expected.len());
        if page["next_cursor"].is_null() {
            break;
        }
        cursor = Some(page["next_cursor"].as_str().unwrap().to_owned());
    }
    assert_eq!(collected, expected);
    let last = expected.last().unwrap()["id"].as_str().unwrap();
    let tail = get_json(
        f.client,
        &format!("{url}?after={last}&limit=100"),
        f.owner,
        200,
    )
    .await;
    assert!(assert_page(&tail, 100).is_empty());
    assert_eq!(tail["next_cursor"], Value::Null);
    let cursor = expected[99]["id"].as_str().unwrap();
    let tail = get_json(
        f.client,
        &format!("{url}?after={cursor}&limit=100"),
        f.owner,
        200,
    )
    .await;
    assert_eq!(assert_page(&tail, 100), expected[100..]);
    assert_eq!(tail["next_cursor"], Value::Null);

    for (query, status) in [
        ("?limit=0", 422),
        ("?limit=101", 422),
        ("?limit=1.5", 422),
        ("?limit=abc", 422),
        ("?after=00000000-0000-0000-0000-000000000000", 422),
        ("?after=11111111-ABCD-4111-8111-111111111111", 422),
        ("?after=11111111abcd41118111111111111111", 422),
        ("?after=invalid", 422),
        ("?unknown=1", 400),
        ("?limit=1&limit=2", 400),
    ] {
        let response = f
            .client
            .get(format!("{url}{query}"))
            .bearer_auth(f.owner)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{query}");
        assert!(
            !response
                .text()
                .await
                .unwrap()
                .contains("directory-private-marker")
        );
    }
    assert!(assert_page(&get_json(f.client, &url, f.foreign, 200).await, 50).is_empty());

    // Reusing a cursor is a fresh read, never a retained authorization receipt.
    let target = own[1];
    sql(
        f.db,
        "UPDATE projects SET owner_id=$2 WHERE id=$1",
        vec![target.into(), f.operator_id.into()],
    )
    .await;
    sql(
        f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![target.into(), f.owner_id.into()],
    )
    .await;
    let target_position = expected
        .iter()
        .position(|p| p["id"] == target.to_string())
        .unwrap();
    let before_target = target_position
        .checked_sub(1)
        .map(|i| expected[i]["id"].as_str().unwrap());
    let replay_url = before_target
        .map(|id| format!("{url}?after={id}&limit=100"))
        .unwrap_or_else(|| format!("{url}?limit=100"));
    let with_member = get_json(f.client, &replay_url, f.owner, 200).await;
    assert!(
        assert_page(&with_member, 100)
            .iter()
            .any(|p| p["id"] == target.to_string())
    );
    sql(
        f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![target.into(), f.owner_id.into()],
    )
    .await;
    let revoked = get_json(f.client, &replay_url, f.owner, 200).await;
    assert!(
        !assert_page(&revoked, 100)
            .iter()
            .any(|p| p["id"] == target.to_string())
    );
    assert!(
        !assert_page(
            &get_json(f.client, &replay_url, "sdlc_pat_read", 200).await,
            100
        )
        .iter()
        .any(|p| p["id"] == target.to_string())
    );

    sql(
        f.db,
        "UPDATE users SET is_active=false WHERE id=$1",
        vec![f.owner_id.into()],
    )
    .await;
    get_json(f.client, &replay_url, f.owner, 403).await;
    get_json(f.client, &replay_url, "sdlc_pat_read", 403).await;
    sql(
        f.db,
        "UPDATE users SET is_active=true WHERE id=$1",
        vec![f.owner_id.into()],
    )
    .await;
    f.unavailable.store(true, Ordering::SeqCst);
    get_json(f.client, &replay_url, f.owner, 503).await;
    get_json(f.client, &replay_url, "sdlc_pat_read", 503).await;
    f.unavailable.store(false, Ordering::SeqCst);

    // Remove only this support fixture's rows; later creation checks retain their original scope.
    for id in own {
        sql(
            f.db,
            "DELETE FROM project_members WHERE project_id=$1",
            vec![id.into()],
        )
        .await;
        sql(
            f.db,
            "DELETE FROM boards WHERE project_id=$1",
            vec![id.into()],
        )
        .await;
        sql(f.db, "DELETE FROM projects WHERE id=$1", vec![id.into()]).await;
    }
    assert_eq!(get_json(f.client, &url, f.owner, 200).await, initial);
}
