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
    let saved = serde_json::to_value(&command).unwrap();
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
