use super::*;
use serde_json::json;
fn command() -> serde_json::Value {
    json!({"expected_owner_version":0,"expected_assignment_version":null,
        "requested_agent_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","idempotency_key":"reserve"})
}
#[test]
fn selector_is_canonical_and_not_a_trusted_runtime_claim() {
    assert!(serde_json::from_value::<ReservePmDraft>(command()).is_ok());
    for selector in [
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        "aaaaaaaaaaaa4aaa8aaaaaaaaaaaaaaa",
        "00000000-0000-0000-0000-000000000000",
    ] {
        let mut c = command();
        c["requested_agent_id"] = json!(selector);
        assert!(serde_json::from_value::<ReservePmDraft>(c).is_err());
    }
    let mut c = command();
    c["machine_subject"] = json!("browser-claim");
    assert!(serde_json::from_value::<ReservePmDraft>(c).is_err());
}
#[test]
fn cas_fields_are_required_and_safe_integers() {
    for field in [
        "expected_owner_version",
        "expected_assignment_version",
        "requested_agent_id",
        "idempotency_key",
    ] {
        let mut c = command();
        c.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<ReservePmDraft>(c).is_err());
    }
    for v in [
        json!(-1),
        json!(9007199254740992i64),
        json!(1.0),
        json!("0"),
        json!(null),
    ] {
        let mut c = command();
        c["expected_owner_version"] = v;
        assert!(serde_json::from_value::<ReservePmDraft>(c).is_err());
    }
    for v in [
        json!(0),
        json!(-1),
        json!(9007199254740992i64),
        json!(1.0),
        json!("1"),
    ] {
        let mut c = command();
        c["expected_assignment_version"] = v;
        assert!(serde_json::from_value::<ReservePmDraft>(c).is_err());
    }
}
#[test]
fn schema_and_deserializer_cannot_advertise_dispatch() {
    let schema =
        serde_json::to_value(<PmDraftReservation as utoipa::PartialSchema>::schema()).unwrap();
    assert_eq!(
        schema["properties"]["dispatch_allowed"]["enum"],
        json!([false])
    );
    assert!(no_dispatch(&mut serde_json::Deserializer::from_str("true")).is_err());
    assert!(!no_dispatch(&mut serde_json::Deserializer::from_str("false")).unwrap());
}
