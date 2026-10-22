//! Real Tracker HTTP with synthetic Auth; no native Forge compatibility claim.
use super::*;

pub async fn verify(
    client: &Client,
    base: &str,
    db: &DatabaseConnection,
    project: Uuid,
    task: Uuid,
    actors: &routing_policy::Actors<'_>,
) {
    let owner = actors.owner;
    let foreign = actors.foreign;
    let administrator = actors.operator;
    let policy = format!("{base}/api/v1/projects/{project}/sdlc/source-policy");
    let binding = format!("{base}/api/v1/issues/{task}/sdlc/source-binding");
    let enrollment = format!("{base}/api/v1/issues/{task}/sdlc/source-enrollment");
    for (path, own_status) in [
        (policy.clone(), 404),
        (format!("{policy}/versions/1"), 404),
        (format!("{policy}/operations/source-http"), 404),
        (binding.clone(), 200),
        (enrollment.clone(), 409),
    ] {
        for (token, expected) in [
            (
                owner,
                if path.contains("/source-policy/operations/") {
                    403
                } else {
                    own_status
                },
            ),
            (
                if path.contains("/source-policy/operations/") {
                    administrator
                } else {
                    owner
                },
                own_status,
            ),
            (foreign, 403),
            ("invalid-source-reader", 401),
        ] {
            let response = client.get(&path).bearer_auth(token).send().await.unwrap();
            assert_eq!(response.status(), expected, "{path}");
            if expected == 200 {
                assert_eq!(response.headers()["cache-control"], "no-store");
                let body: Value = response.json().await.unwrap();
                assert_eq!(body["state"], "blocked");
                assert!(body["binding"].is_null());
            }
        }
    }
    let command = json!({"expected_source_revision":null,"source":{
        "forge_instance_id":Uuid::new_v4(),"repository_id":Uuid::new_v4(),
        "base_ref":"refs/heads/main","base_commit":"a".repeat(40)},"idempotency_key":"source-http"});
    // No active namespace/registered Forge: do not invent a successful verification.
    for (token, expected) in [
        (owner, 403),
        (administrator, 409),
        (foreign, 403),
        ("sdlc_pat_owner", 403),
    ] {
        post(client, &policy, token, &command, expected).await;
    }
    let mut injected = command;
    injected["verification"] = json!({"fixture":"untrusted caller"});
    post(client, &policy, administrator, &injected, 422).await;
    let enroll = json!({"expected_preflight_sha256":"a".repeat(64),"source_revision":1,
        "policy_hash":"b".repeat(64),"idempotency_key":"enrollment-http"});
    for (token, expected) in [(owner, 409), (foreign, 403), ("sdlc_pat_owner", 403)] {
        post(client, &enrollment, token, &enroll, expected).await;
    }
    for table in [
        "sdlc_source_policy_revisions",
        "sdlc_source_policy_operations",
        "sdlc_draft_source_enrollments",
        "sdlc_task_source_bindings",
    ] {
        assert_eq!(
            count(db, table).await,
            0,
            "rejected HTTP must not publish {table}"
        );
    }
}
