//! Sealed role artifacts use existing attachment metadata, never caller filenames as authority.
use crate::sdlc_lifecycle::{ArtifactReceipt, DeliveryRole};
use serde::{Deserialize, Serialize};
use shared::{AppError, AttachmentId};
use uuid::Uuid;

pub const SEALED_PREFIX: &str = "sdlc-artifact-v1_";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactBinding {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub role: DeliveryRole,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachmentArtifact {
    pub receipt: ArtifactReceipt,
    pub binding: ArtifactBinding,
}

pub struct ArtifactUpload {
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seal {
    pub binding: ArtifactBinding,
    pub run_id: Uuid,
    pub sha256: String,
    pub artifact_id: Uuid,
}

pub fn is_sealed(key: &str) -> bool {
    key.starts_with(SEALED_PREFIX)
}

impl Seal {
    pub fn new(
        task: Uuid,
        binding: ArtifactBinding,
        run_id: Uuid,
        file_name: &str,
        content_type: &str,
        sha256: String,
    ) -> Result<Self, AppError> {
        let identity = serde_json::to_vec(&(
            "tracker-role-artifact-v1",
            task,
            &binding,
            run_id,
            file_name,
            content_type,
            &sha256,
        ))
        .map_err(AppError::internal)?;
        let seal = Self {
            binding,
            run_id,
            sha256,
            artifact_id: Uuid::new_v5(&Uuid::NAMESPACE_URL, &identity),
        };
        Self::parse(&seal.key())?.ok_or_else(|| AppError::conflict("invalid artifact seal"))
    }

    pub fn key(&self) -> String {
        format!(
            "{SEALED_PREFIX}{}_{}_{}_{}_{}_{}",
            self.binding.assignment_id,
            self.binding.execution_id,
            self.run_id,
            self.binding.role.key(),
            self.sha256,
            self.artifact_id
        )
    }

    pub fn parse(key: &str) -> Result<Option<Self>, AppError> {
        let Some(rest) = key.strip_prefix(SEALED_PREFIX) else {
            return Ok(None);
        };
        let bad = || AppError::conflict("sealed attachment metadata invalid");
        let parts: Vec<_> = rest.split('_').collect();
        if parts.len() != 6
            || parts[4].len() != 64
            || !parts[4]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(bad());
        }
        let uuid = |raw: &str| -> Result<Uuid, AppError> {
            let id: Uuid = raw.parse().map_err(|_| bad())?;
            if id.is_nil() || id.to_string() != raw {
                return Err(bad());
            }
            Ok(id)
        };
        let role = match parts[3] {
            "analyst" => DeliveryRole::Analyst,
            "architect" => DeliveryRole::Architect,
            "developer" => DeliveryRole::Developer,
            "reviewer" => DeliveryRole::Reviewer,
            "tester" => DeliveryRole::Tester,
            "devops" => DeliveryRole::DevOps,
            _ => return Err(bad()),
        };
        Ok(Some(Self {
            binding: ArtifactBinding {
                assignment_id: uuid(parts[0])?,
                execution_id: uuid(parts[1])?,
                role,
            },
            run_id: uuid(parts[2])?,
            sha256: parts[4].into(),
            artifact_id: uuid(parts[5])?,
        }))
    }

    pub fn artifact(&self) -> AttachmentArtifact {
        AttachmentArtifact {
            receipt: ArtifactReceipt {
                reference: format!("tracker-attachment:{}", self.artifact_id),
                sha256: self.sha256.clone(),
            },
            binding: self.binding.clone(),
        }
    }

    pub fn validate_attachment(&self, attachment: &crate::Attachment) -> Result<(), AppError> {
        let expected = Self::new(
            attachment.issue_id.as_uuid(),
            self.binding.clone(),
            self.run_id,
            attachment.file_name.as_ref(),
            attachment.content_type.as_ref(),
            self.sha256.clone(),
        )?;
        if &expected != self || attachment.id != AttachmentId::from_uuid(self.artifact_id) {
            return Err(AppError::conflict("sealed attachment identity mismatch"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_six_delivery_roles_round_trip_and_run_identity_is_bound() {
        let task = Uuid::new_v4();
        let assignment = Uuid::new_v4();
        let execution = Uuid::new_v4();
        for role in [
            DeliveryRole::Analyst,
            DeliveryRole::Architect,
            DeliveryRole::Developer,
            DeliveryRole::Reviewer,
            DeliveryRole::Tester,
            DeliveryRole::DevOps,
        ] {
            let binding = ArtifactBinding {
                assignment_id: assignment,
                execution_id: execution,
                role,
            };
            let seal = Seal::new(
                task,
                binding.clone(),
                Uuid::new_v4(),
                "result.json",
                "application/json",
                "a".repeat(64),
            )
            .unwrap();
            assert_eq!(Seal::parse(&seal.key()).unwrap(), Some(seal.clone()));
            let other_run = Seal::new(
                task,
                binding,
                Uuid::new_v4(),
                "result.json",
                "application/json",
                "a".repeat(64),
            )
            .unwrap();
            assert_ne!(seal.artifact_id, other_run.artifact_id);
        }
    }
    #[test]
    fn exact_replay_has_stable_identity_and_closed_binding() {
        let task = Uuid::new_v4();
        let binding = ArtifactBinding {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            role: DeliveryRole::Reviewer,
        };
        let run = Uuid::new_v4();
        let seal = Seal::new(
            task,
            binding.clone(),
            run,
            "report.json",
            "application/json",
            "a".repeat(64),
        )
        .unwrap();
        assert_eq!(Seal::parse(&seal.key()).unwrap(), Some(seal.clone()));
        assert_eq!(
            seal,
            Seal::new(
                task,
                binding.clone(),
                run,
                "report.json",
                "application/json",
                "a".repeat(64)
            )
            .unwrap()
        );
        for other in [
            Seal::new(
                task,
                binding.clone(),
                run,
                "other.json",
                "application/json",
                "a".repeat(64),
            )
            .unwrap(),
            Seal::new(
                task,
                binding,
                run,
                "report.json",
                "application/json",
                "b".repeat(64),
            )
            .unwrap(),
        ] {
            assert_ne!(seal.artifact_id, other.artifact_id);
        }
        assert!(seal.key().len() < 255);
        assert!(!seal.key().contains(['/', '\\', ':']));
        assert_eq!(Seal::parse("old-attachment-report.json").unwrap(), None);
        assert!(Seal::parse(SEALED_PREFIX).is_err());
        assert!(Seal::parse(&seal.key().replace(&seal.sha256, &"G".repeat(64))).is_err());
        assert!(serde_json::from_value::<AttachmentArtifact>(serde_json::json!({"receipt":seal.artifact().receipt,"binding":seal.binding,"extra":true})).is_err());
    }
}
