use domain::sdlc_metadata::MetadataPage;
use reqwest::Client;
use serde_json::{Value, json};

pub fn save(name: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("TT_SDLC_METADATA_GOLDEN_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(std::path::Path::new(&directory).join(name), bytes).unwrap();
    }
}

pub async fn get(client: &Client, url: &str, token: &str, status: u16) -> (Vec<u8>, Value) {
    let response = client.get(url).bearer_auth(token).send().await.unwrap();
    let actual = response.status().as_u16();
    let bytes = response.bytes().await.unwrap().to_vec();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(actual, status, "{url}: {value}");
    (bytes, value)
}

fn keys(value: &Value, expected: &[&str]) {
    let mut actual: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected = expected.to_vec();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

pub fn verify(page: &Value, legacy: &Value) {
    let _: MetadataPage = serde_json::from_value(page.clone()).unwrap();
    keys(
        page,
        &[
            "contract_version",
            "projection",
            "after",
            "next_after",
            "has_more",
            "events",
        ],
    );
    assert_eq!(page["contract_version"], 1);
    assert_eq!(page["projection"], "metadata_v1");
    for (event, original) in page["events"]
        .as_array()
        .unwrap()
        .iter()
        .zip(legacy["events"].as_array().unwrap())
    {
        keys(
            event,
            &[
                "sequence",
                "event_id",
                "task_id",
                "event_type",
                "created_at",
                "metadata_sha256",
                "payload",
            ],
        );
        assert_eq!(
            event["sequence"],
            original["sequence"].as_i64().unwrap().to_string()
        );
        assert_eq!(event["event_id"], original["event_id"]);
        assert_eq!(event["task_id"], original["task_id"]);
        assert_eq!(event["event_type"], original["event_type"]);
        let mut canonical = event.clone();
        let digest = canonical
            .as_object_mut()
            .unwrap()
            .remove("metadata_sha256")
            .unwrap();
        assert_eq!(
            digest,
            app::sdlc::canonical_hash(
                &json!({"contract_version":1,"projection":"metadata_v1","event":canonical})
            )
            .unwrap()
        );
        let payload = &event["payload"];
        keys(
            payload,
            &[
                "tracker_instance_id",
                "project_id",
                "root_task_id",
                "owner_subject",
                "stage",
                "current_requirement_revision",
                "resource",
            ],
        );
        for field in [
            "tracker_instance_id",
            "project_id",
            "root_task_id",
            "owner_subject",
            "stage",
        ] {
            assert_eq!(payload[field], original["payload"][field]);
        }
        assert_eq!(
            payload["current_requirement_revision"],
            original["payload"]["requirement_revision"]
        );
        let r = &payload["resource"];
        let result = &original["payload"]["result"];
        match event["event_type"].as_str().unwrap() {
            "task.created" => {
                keys(r, &["input"]);
                if !r["input"].is_null() {
                    keys(&r["input"], &["snapshot_ref", "sha256"]);
                }
            }
            "task.bound" => keys(r, &[]),
            "pm.assigned" => {
                keys(
                    r,
                    &[
                        "assignment_id",
                        "execution_id",
                        "agent_id",
                        "assignment_version",
                    ],
                );
                for field in ["assignment_id", "execution_id", "agent_id"] {
                    assert_eq!(r[field], result[field]);
                }
                assert_eq!(r["assignment_version"], result["version"]);
            }
            "clarification.published" | "clarification.cancelled" => {
                keys(
                    r,
                    &[
                        "question_id",
                        "question_version",
                        "request_id",
                        "checkpoint_id",
                        "requirement_revision",
                        "state",
                        "fence",
                    ],
                );
                assert_eq!(r["question_id"], result["id"]);
                assert_eq!(r["question_version"], result["version"]);
                for field in [
                    "request_id",
                    "checkpoint_id",
                    "requirement_revision",
                    "state",
                ] {
                    assert_eq!(r[field], result[field]);
                }
                for field in [
                    "assignment_id",
                    "execution_id",
                    "agent_id",
                    "assignment_version",
                ] {
                    assert_eq!(r["fence"][field], result[field]);
                }
            }
            "clarification.answered" => {
                keys(
                    r,
                    &[
                        "answer_id",
                        "question_id",
                        "question_version",
                        "request_id",
                        "checkpoint_id",
                        "requirement_revision",
                        "fence",
                    ],
                );
                assert_eq!(r["answer_id"], result["id"]);
                for field in ["question_id", "question_version", "requirement_revision"] {
                    assert_eq!(r[field], result[field]);
                }
                let q = legacy["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| {
                        e["event_type"] == "clarification.published"
                            && e["payload"]["result"]["id"] == result["question_id"]
                            && e["payload"]["result"]["version"] == result["question_version"]
                    })
                    .unwrap();
                for field in ["request_id", "checkpoint_id"] {
                    assert_eq!(r[field], q["payload"]["result"][field]);
                }
                for field in [
                    "assignment_id",
                    "execution_id",
                    "agent_id",
                    "assignment_version",
                ] {
                    assert_eq!(r["fence"][field], q["payload"]["result"][field]);
                }
            }
            "requirements.published" => {
                keys(r, &["requirement_revision", "content_hash"]);
                assert_eq!(r["requirement_revision"], result["revision"]);
                assert_eq!(r["content_hash"], result["content_hash"]);
            }
            "requirements.evidence_recorded" => {
                keys(
                    r,
                    &["requirement_revision", "content_hash", "check_id_sha256"],
                );
                assert_eq!(r["requirement_revision"], result["requirement_revision"]);
                assert_eq!(r["content_hash"], result["content_hash"]);
                assert_eq!(
                    r["check_id_sha256"],
                    app::sdlc::canonical_hash(&result["check_id"]).unwrap()
                );
            }
            "requirements.confirmed" => {
                keys(
                    r,
                    &["confirmation_id", "requirement_revision", "content_hash"],
                );
                assert_eq!(r["confirmation_id"], result["id"]);
                assert_eq!(r["requirement_revision"], result["revision"]);
                assert_eq!(r["content_hash"], result["content_hash"]);
            }
            other => panic!("unsupported event: {other}"),
        }
    }
}
