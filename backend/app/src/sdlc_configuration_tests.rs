use super::*;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) fn fixture() -> (RoleRoute, Value) {
    let route = RoleRoute {
        agent_id: Uuid::new_v4(),
        fleet_config_revision: 2,
        package_commit: domain::sdlc_routing::PACKAGE_COMMIT.into(),
        package_manifest_sha256: "a".repeat(64),
        namespace_id: "2".into(),
        namespace_name: "hermes-analyst".into(),
        workflow_id: "2".into(),
        workflow_key: "hermes-sdlc:analyst".into(),
        profile: "hermes-sdlc-analyst".into(),
        workflow_catalog_version: 3,
        workflow_catalog_sha256: "b".repeat(64),
    };
    let value = json!({"contract_version":1,"observation_ref":Uuid::new_v4(),"agent_id":route.agent_id,
        "sdlc_role":"analyst","effective_revision":2,"managed_files_verified":true,"runtime_ready":false,
        "observed_at":Utc::now(),"blockers":["runtime_skill_inventory_not_verified","workflow_assignment_protocol_not_verified"],
        "package":{"schema":"base-sdlc/package-proof/v1","repository":"https://github.com/FerrPOINT/services-base.git",
          "commit":route.package_commit,"manifestSha256":route.package_manifest_sha256,"role":"analyst",
          "namespace":route.namespace_name,"profile":route.profile,"modes":["analysis"],
          "roleInstructionSha256":"c".repeat(64),"skillSha256":{"acceptance-test-design":"d".repeat(64)}},
        "workflow_binding":{"schema":"base-sdlc/workflow-binding/v1","namespace_id":route.namespace_id,
          "namespace_name":route.namespace_name,"workflow_id":route.workflow_id,"workflow_key":route.workflow_key,
          "role_key":"analyst","profile":route.profile,"catalog_version":3,
          "catalog_sha256":route.workflow_catalog_sha256,"skills_revision":route.package_commit,"runtime_ready":false}});
    (route, value)
}

#[test]
fn actual_fleet_contract_matches_declared_route_but_not_execution() {
    let (route, value) = fixture();
    assert!(
        compare(
            &route,
            "analyst",
            "analysis",
            serde_json::from_value(value).unwrap()
        )
        .is_ok()
    );
}

#[test]
fn configuration_drift_readiness_and_stale_proof_are_rejected() {
    let (route, value) = fixture();
    for (path, changed) in [
        ("/contract_version", json!(2)),
        ("/observation_ref", json!(Uuid::nil())),
        ("/agent_id", json!(Uuid::new_v4())),
        ("/sdlc_role", json!("architect")),
        ("/effective_revision", json!(3)),
        ("/managed_files_verified", json!(false)),
        ("/runtime_ready", json!(true)),
        ("/blockers", json!([])),
        ("/package/commit", json!("f".repeat(40))),
        ("/package/manifestSha256", json!("e".repeat(64))),
        ("/package/role", json!("architect")),
        ("/package/namespace", json!("other")),
        ("/package/profile", json!("other")),
        ("/package/modes", json!([])),
        ("/package/modes", json!(["development"])),
        ("/package/roleInstructionSha256", json!("invalid")),
        ("/package/skillSha256", json!({})),
        ("/package/schema", json!("base-sdlc/package-proof/v2")),
        ("/workflow_binding/namespace_id", json!("3")),
        ("/workflow_binding/namespace_name", json!("other")),
        ("/workflow_binding/workflow_id", json!("3")),
        ("/workflow_binding/workflow_key", json!("other")),
        ("/workflow_binding/role_key", json!("architect")),
        ("/workflow_binding/profile", json!("other")),
        ("/workflow_binding/catalog_version", json!(2)),
        ("/workflow_binding/catalog_sha256", json!("e".repeat(64))),
        ("/workflow_binding/skills_revision", json!("f".repeat(40))),
        ("/workflow_binding/runtime_ready", json!(true)),
        (
            "/observed_at",
            json!(Utc::now() - chrono::Duration::seconds(6)),
        ),
        (
            "/observed_at",
            json!(Utc::now() + chrono::Duration::seconds(3)),
        ),
    ] {
        let mut broken = value.clone();
        *broken.pointer_mut(path).unwrap() = changed;
        assert!(
            compare(
                &route,
                "analyst",
                "analysis",
                serde_json::from_value(broken).unwrap()
            )
            .is_err(),
            "{path}"
        );
    }
    let mut unknown = value;
    unknown["caller_ready"] = json!(true);
    assert!(serde_json::from_value::<Observation>(unknown).is_err());
}

#[test]
fn matched_observation_still_requires_freshness_after_repository_readback() {
    let mut matched = MatchedConfiguration {
        observation_ref: Uuid::new_v4(),
        observed_at: Utc::now(),
    };
    assert!(matched.ensure_fresh().is_ok());
    matched.observed_at = Utc::now() - chrono::Duration::seconds(6);
    assert!(matched.ensure_fresh().is_err());
    matched.observed_at = Utc::now() + chrono::Duration::seconds(3);
    assert!(matched.ensure_fresh().is_err());
}

#[tokio::test]
async fn chunked_body_without_content_length_still_has_a_hard_limit() {
    let (route, _) = fixture();
    let body = "x".repeat(65_537);
    let (origin, task) = serve(format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
        body.len()
    ))
    .await;
    let reader =
        FleetConfigurationReader::new(&origin, &format!("sdlc_pat_{}", "a".repeat(64))).unwrap();
    assert!(reader.read(&route, "analyst", "analysis").await.is_err());
    task.await.unwrap();
}

#[test]
fn operator_origin_and_token_cannot_redirect_or_embed_credentials() {
    let token = format!("sdlc_pat_{}", "a".repeat(64));
    assert!(FleetConfigurationReader::new("http://fleet:7811", &token).is_ok());
    for origin in [
        "http://user@fleet",
        "http://fleet/path",
        "http://fleet?token=1",
        "http://fleet#x",
        "http://fleet\\evil",
        "ftp://fleet",
        " http://fleet",
    ] {
        assert!(
            FleetConfigurationReader::new(origin, &token).is_err(),
            "{origin}"
        );
    }
    for token in [
        "local_jwt",
        "sdlc_pat_short",
        "sdlc_pat_secret\nheader:value",
    ] {
        assert!(FleetConfigurationReader::new("http://fleet", token).is_err());
    }
}

async fn serve(response: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert!(request.len() < 8192);
            stream.read_exact(&mut byte).await.unwrap();
            request.push(byte[0]);
        }
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request).unwrap()
    });
    (origin, task)
}

#[tokio::test]
async fn http_reads_real_counterpart_path_with_only_reader_credential() {
    let (route, value) = fixture();
    let body = value.to_string();
    let (origin, task) = serve(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())).await;
    let token = format!("sdlc_pat_{}", "a".repeat(64));
    let reader = FleetConfigurationReader::new(&origin, &token).unwrap();
    assert!(reader.read(&route, "analyst", "analysis").await.is_ok());
    let request = task.await.unwrap();
    assert!(request.starts_with(&format!(
        "GET /internal/runtime/v1/agents/{}/configuration HTTP/1.1",
        route.agent_id
    )));
    assert!(request.contains(&format!("authorization: Bearer {token}")));
    assert!(request.contains("cache-control: no-cache, no-store"));
}

#[tokio::test]
async fn http_rejects_redirect_errors_oversize_encoding_and_invalid_json() {
    let (route, _) = fixture();
    for response in [
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/steal\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nxxx",
    ] {
        let (origin, task) = serve(response.into()).await;
        let reader =
            FleetConfigurationReader::new(&origin, &format!("sdlc_pat_{}", "a".repeat(64)))
                .unwrap();
        assert!(reader.read(&route, "analyst", "analysis").await.is_err());
        task.await.unwrap();
    }
}
