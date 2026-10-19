use super::*;
use serde_json::json;
fn claim() -> serde_json::Value {
    json!({"expected_owner_version":1,"fence":{
        "assignment_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "execution_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        "agent_id":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","assignment_version":1},
        "idempotency_key":"claim"})
}
#[test]
fn lease_commands_are_strict_canonical_and_positive_safe_cas() {
    assert!(serde_json::from_value::<ClaimExecutionLease>(claim()).is_ok());
    for field in ["expected_owner_version", "fence", "idempotency_key"] {
        let mut c = claim();
        c.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
    }
    for n in [
        json!(0),
        json!(-1),
        json!(9007199254740992i64),
        json!(1.0),
        json!("1"),
    ] {
        let mut c = claim();
        c["expected_owner_version"] = n;
        assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
    }
    for id in [
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        "00000000-0000-0000-0000-000000000000",
    ] {
        let mut c = claim();
        c["fence"]["assignment_id"] = json!(id);
        assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
    }
    let mut c = claim();
    c["ttl_seconds"] = json!(60);
    assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
    assert!(no_dispatch(&mut serde_json::Deserializer::from_str("true")).is_err());
}
