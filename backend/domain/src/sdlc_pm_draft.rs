use crate::sdlc::{MAX_SAFE_VERSION, PmAssignment};
use crate::sdlc_metadata::MetadataInputRef;
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use uuid::Uuid;

fn owner_version<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let n = i64::deserialize(d)?;
    if (0..=MAX_SAFE_VERSION).contains(&n) {
        Ok(n)
    } else {
        Err(D::Error::custom("invalid owner version"))
    }
}
fn assignment_version<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let n = Option::<i64>::deserialize(d)?;
    if n.is_none_or(|n| (1..=MAX_SAFE_VERSION).contains(&n)) {
        Ok(n)
    } else {
        Err(D::Error::custom("invalid assignment version"))
    }
}
fn selector<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
    let raw = String::deserialize(d)?;
    let id = Uuid::parse_str(&raw).map_err(D::Error::custom)?;
    if id.is_nil() || id.to_string() != raw {
        Err(D::Error::custom("invalid agent selector"))
    } else {
        Ok(id)
    }
}
fn no_dispatch<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    if bool::deserialize(d)? {
        Err(D::Error::custom("reservation cannot authorize dispatch"))
    } else {
        Ok(false)
    }
}
fn no_dispatch_schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Boolean)
        .enum_values(Some([false]))
        .into()
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReservePmDraft {
    #[serde(deserialize_with = "owner_version")]
    #[schema(minimum = 0, maximum = 9007199254740991i64)]
    pub expected_owner_version: i64,
    #[serde(deserialize_with = "assignment_version")]
    #[schema(
        required = true,
        nullable = true,
        minimum = 1,
        maximum = 9007199254740991i64
    )]
    pub expected_assignment_version: Option<i64>,
    #[serde(deserialize_with = "selector")]
    pub requested_agent_id: Uuid,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PmDraftBinding {
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OwnerCas {
    #[schema(minimum = 0, maximum = 9007199254740991i64)]
    pub expected_version: i64,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub version: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmDraftExecution {
    #[schema(pattern = "^[1-9][0-9]{0,18}$")]
    pub ordinal: String,
    #[schema(pattern = "^SDLC-[1-9][0-9]{0,18}$")]
    pub key: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub enum PmDraftVariant {
    #[serde(rename = "pm_draft_reserved")]
    Reserved,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub enum PmDraftAdmissionState {
    #[serde(rename = "reserved")]
    Reserved,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmDraftReservation {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub variant: PmDraftVariant,
    pub binding: PmDraftBinding,
    pub owner_cas: OwnerCas,
    pub assignment: PmAssignment,
    pub execution: PmDraftExecution,
    pub input: MetadataInputRef,
    pub assignment_operation_key: String,
    pub admission_state: PmDraftAdmissionState,
    #[serde(deserialize_with = "no_dispatch")]
    #[schema(schema_with = no_dispatch_schema)]
    pub dispatch_allowed: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmDraftOperation {
    pub idempotency_key: String,
    pub request_sha256: String,
    pub result: PmDraftReservation,
}
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmDraftReadback {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub binding: PmDraftBinding,
    #[schema(minimum = 0, maximum = 9007199254740991i64)]
    pub owner_version: i64,
    #[schema(required = true, nullable = true)]
    pub current: Option<PmDraftReservation>,
    #[schema(required = true, nullable = true)]
    pub operation: Option<PmDraftOperation>,
}

#[cfg(test)]
mod tests {
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
}
