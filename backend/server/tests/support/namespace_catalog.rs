use super::*;

pub fn tokens(tokens: &mut HashMap<String, Value>) {
    for (name, subject, scopes) in [
        (
            "sdlc_pat_namespace_reader",
            "namespace-reader",
            vec!["task-tracker:read"],
        ),
        (
            "sdlc_pat_namespace_writer",
            "namespace-reader",
            vec!["task-tracker:write"],
        ),
        (
            "sdlc_pat_namespace_foreign",
            "fleet",
            vec!["task-tracker:read"],
        ),
    ] {
        tokens.insert(
            name.into(),
            json!({"sub":subject,"email":format!("{subject}@example.test"),"scopes":scopes}),
        );
    }
}
pub async fn verify(
    client: &Client,
    base: &str,
    db: &DatabaseConnection,
    source_project: Uuid,
    human: &str,
    administrator: &str,
) {
    use shared::resource_context::{NamespaceRef, OwnerCommand, ResourceKind, ResourceRef};
    let project = Uuid::new_v4();
    sql(db, "INSERT INTO projects(id,key,name,owner_id,default_board_id) SELECT $1,'READR','Reader fixture',owner_id,$2 FROM projects WHERE id=$3", vec![project.into(), Uuid::new_v4().into(), source_project.into()]).await;
    let registry: Uuid = std::env::var("TT_NAMESPACE__REGISTRY_INSTANCE_ID")
        .unwrap()
        .parse()
        .unwrap();
    let instance: Uuid = std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .unwrap()
        .parse()
        .unwrap();
    let namespace = Uuid::new_v4();
    let command = OwnerCommand {
        schema_version: 1,
        namespace: NamespaceRef {
            registry_instance_id: registry,
            namespace_id: namespace,
        },
        resource: ResourceRef {
            kind: ResourceKind::TrackerProject,
            instance_id: instance,
            resource_id: project,
        },
        operation_id: Uuid::new_v4(),
        generation: 1,
        state: "active".into(),
        create_spec: None,
    };
    let mut saved = serde_json::to_value(&command).unwrap();
    sql(db,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",vec![project.into(),registry.into(),namespace.into(),saved.clone().into()]).await;
    sql(
        db,
        "UPDATE projects SET namespace_managed=true WHERE id=$1",
        vec![project.into()],
    )
    .await;
    let before = count(db, "users").await;
    let path = format!("{base}/api/v1/namespace-projects?limit=1&offset=0");
    let response = client
        .get(&path)
        .bearer_auth("sdlc_pat_namespace_reader")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let page: Value = response.json().await.unwrap();
    assert_eq!(page.as_array().unwrap().len(), 1);
    assert_eq!(
        page[0]["binding"]["resource"]["resource_id"],
        project.to_string()
    );
    assert_eq!(
        page[0]["binding"]["namespace"]["namespace_id"],
        namespace.to_string()
    );
    assert_eq!(page[0]["label"], "Reader fixture");
    assert_eq!(page[0]["resource_key"], "READR");
    let next: Value = client
        .get(format!("{base}/api/v1/namespace-projects?limit=1&offset=1"))
        .bearer_auth("sdlc_pat_namespace_reader")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(next.as_array().unwrap().is_empty());
    for (token, status) in [
        (human, 403),
        ("sdlc_pat_namespace_writer", 403),
        ("sdlc_pat_namespace_foreign", 403),
        ("sdlc_pat_revoked_namespace_reader", 401),
    ] {
        assert_eq!(
            client
                .get(&path)
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    assert_eq!(
        client
            .get(format!("{base}/api/v1/projects"))
            .bearer_auth("sdlc_pat_namespace_reader")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        count(db, "users").await,
        before,
        "machine catalog reads never create human profiles"
    );
    // Exercise the real project-scoped route, not the task-scoped compatibility API.
    let forge = Uuid::new_v4();
    sql(db, "INSERT INTO project_repository_links(project_id,forge_instance_id,repository_id,snapshot,created_by) SELECT $1,$2,md5(n::text)::uuid,jsonb_build_object('namespace',$3::jsonb,'forge_instance_id',$2::uuid,'repository_id',md5(n::text)::uuid,'public_name','repository-'||n,'kind','hosted'),p.owner_id FROM projects p CROSS JOIN generate_series(1,102) n WHERE p.id=$1",
        vec![project.into(),forge.into(),serde_json::to_value(&command.namespace).unwrap().into()]).await;
    let links_url = format!("{base}/api/v1/projects/READR/repositories");
    let first = client
        .get(&links_url)
        .bearer_auth(human)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    let first: Vec<Value> = first.json().await.unwrap();
    assert_eq!(first.len(), 100);
    let last = client
        .get(format!("{links_url}?offset=100"))
        .bearer_auth(human)
        .send()
        .await
        .unwrap();
    assert_eq!(last.status(), 200);
    let last: Vec<Value> = last.json().await.unwrap();
    assert_eq!(last.len(), 2);
    let ids: std::collections::HashSet<_> = first
        .iter()
        .chain(&last)
        .map(|row| row["repository_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 102);
    assert_eq!(client.get(&links_url).send().await.unwrap().status(), 401);
    for offset in ["-1", "4294967296", "invalid"] {
        assert_eq!(
            client
                .get(format!("{links_url}?offset={offset}"))
                .bearer_auth(human)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        client
            .get(&links_url)
            .bearer_auth("sdlc_pat_namespace_foreign")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let (curl_status, curl_page) = curl_get_links(&format!("{links_url}?offset=100"), human).await;
    assert_eq!(curl_status, 200);
    assert_eq!(curl_page.as_array().unwrap().len(), 2);
    let available = format!("{base}/api/v1/projects/READR/available-repositories");
    assert_eq!(
        client
            .get(available)
            .bearer_auth(human)
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    let reference = json!({"forge_instance_id":forge,"repository_id":Uuid::new_v4()});
    // A reader cannot change repository links even when the catalog is unavailable.
    for (token, expected) in [(human, 403), (administrator, 503)] {
        assert_eq!(
            client
                .post(&links_url)
                .bearer_auth(token)
                .json(&reference)
                .send()
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    let remove = format!(
        "{links_url}/{forge}/{}",
        first[0]["repository_id"].as_str().unwrap()
    );
    assert_eq!(
        client
            .delete(&remove)
            .bearer_auth("sdlc_pat_namespace_foreign")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let mut archived = saved.clone();
    archived["state"] = json!("archived");
    archived["generation"] = json!(2);
    archived["operation_id"] = json!(Uuid::new_v4());
    sql(db,"UPDATE tracker_namespace_bindings SET state='archived',generation=2,command=$2 WHERE resource_id=$1",
        vec![project.into(),archived.clone().into()]).await;
    let missing = format!("{links_url}/{forge}/{}", Uuid::new_v4());
    for url in [&remove, &missing] {
        assert_eq!(
            client
                .delete(url)
                .bearer_auth(administrator)
                .send()
                .await
                .unwrap()
                .status(),
            409
        );
    }
    assert_eq!(
        client
            .post(&links_url)
            .bearer_auth(human)
            .json(&reference)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        client
            .get(&links_url)
            .bearer_auth(human)
            .send()
            .await
            .unwrap()
            .json::<Vec<Value>>()
            .await
            .unwrap()
            .len(),
        100
    );
    saved = archived;
    saved["state"] = json!("active");
    saved["generation"] = json!(3);
    saved["operation_id"] = json!(Uuid::new_v4());
    sql(db,"UPDATE tracker_namespace_bindings SET state='active',generation=3,command=$2 WHERE resource_id=$1",
        vec![project.into(),saved.clone().into()]).await;
    assert_eq!(
        client
            .delete(&remove)
            .bearer_auth(human)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .delete(&remove)
            .bearer_auth(human)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    println!(
        "PROJECT_LINKS_HTTP_AND_CURL 100+2 scoped pages, invalid offsets, unauthenticated and foreign denial passed"
    );
    sql(
        db,
        "DELETE FROM project_repository_links WHERE project_id=$1",
        vec![project.into()],
    )
    .await;
    let mut corrupt = saved.clone();
    corrupt["resource"]["instance_id"] = json!(Uuid::new_v4());
    sql(
        db,
        "UPDATE tracker_namespace_bindings SET command=$2 WHERE resource_id=$1",
        vec![project.into(), corrupt.into()],
    )
    .await;
    assert_eq!(
        client
            .get(&path)
            .bearer_auth("sdlc_pat_namespace_reader")
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    sql(
        db,
        "UPDATE tracker_namespace_bindings SET command=$2 WHERE resource_id=$1",
        vec![project.into(), saved.into()],
    )
    .await;
    assert_eq!(
        client
            .get(&path)
            .bearer_auth("sdlc_pat_namespace_reader")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}

async fn curl_get_links(url: &str, token: &str) -> (u16, Value) {
    let (url, token) = (url.to_owned(), token.to_owned());
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("curl")
            .args([
                "--silent",
                "--show-error",
                "--max-time",
                "10",
                "--write-out",
                "\n%{http_code}",
                "--header",
                &format!("Authorization: Bearer {token}"),
                "--url",
                &url,
            ])
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let (body, status) = stdout.rsplit_once('\n').unwrap();
    (status.parse().unwrap(), serde_json::from_str(body).unwrap())
}
