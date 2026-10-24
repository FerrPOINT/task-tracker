//! Administrative identity metadata, separate from admission and content access.
use serde::{Deserialize, Serialize};
use shared::{AppError, Timestamp, space_access::SpaceRef};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentityProjectMembership {
    pub project_id: Uuid,
    pub project_key: String,
    pub project_name: String,
    pub role: String,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub joined_at: Option<Timestamp>,
    pub spaces: Vec<SpaceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentityMembershipPage {
    pub items: Vec<IdentityProjectMembership>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

pub fn validate_page(subject: Uuid, limit: u64, offset: u64) -> Result<(), AppError> {
    if subject.is_nil() || !(1..=100).contains(&limit) || offset > i64::MAX as u64 {
        return Err(AppError::invalid_input("invalid identity membership page"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_schema_exposes_nullable_rfc3339_join_time() {
        let schema =
            serde_json::to_value(<IdentityProjectMembership as utoipa::PartialSchema>::schema())
                .unwrap();
        assert_eq!(schema["properties"]["joined_at"]["format"], "date-time");
        assert!(
            !schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|name| name == "joined_at")
        );
    }

    #[test]
    fn page_cannot_be_unbounded_or_target_nil_identity() {
        assert!(validate_page(Uuid::from_u128(1), 25, 0).is_ok());
        assert!(validate_page(Uuid::nil(), 25, 0).is_err());
        assert!(validate_page(Uuid::from_u128(1), 0, 0).is_err());
        assert!(validate_page(Uuid::from_u128(1), 101, 0).is_err());
        assert!(validate_page(Uuid::from_u128(1), 25, u64::MAX).is_err());
    }
}
