//! Exact paired-source GET on the existing registered namespace repository reader.
use super::*;

fn decode_source(
    bytes: &[u8],
    instance: Uuid,
    namespace: &NamespaceRef,
    source: &domain::sdlc_source::SourcePolicySpec,
) -> Result<domain::sdlc_source::VerifiedSource, AppError> {
    let observed: domain::sdlc_source::VerifiedSource = serde_json::from_slice(bytes)
        .map_err(|_| AppError::Unavailable("forge_source_invalid_response".into()))?;
    if observed.forge_instance_id != instance {
        return Err(AppError::Unavailable(
            "forge_source_instance_mismatch".into(),
        ));
    }
    app::sdlc_source::validate_verification(source, namespace, &observed, chrono::Utc::now())?;
    Ok(observed)
}

impl ForgeReader {
    pub(super) async fn verify_source_observation(
        &self,
        namespace: &NamespaceRef,
        source: &domain::sdlc_source::SourcePolicySpec,
    ) -> Result<domain::sdlc_source::VerifiedSource, AppError> {
        app::sdlc_source::validate_spec(source)?;
        if source.forge_instance_id != self.instance {
            return Err(AppError::invalid_input("unregistered_repository_ref"));
        }
        let endpoint = self
            .endpoint
            .join(&format!(
                "api/v1/namespace-repositories/{}",
                source.repository_id
            ))
            .map_err(|_| AppError::invalid_input("invalid_repository_ref"))?;
        let mut response = self
            .client
            .get(endpoint)
            .query(&[
                (
                    "registry_instance_id",
                    namespace.registry_instance_id.to_string(),
                ),
                ("namespace_id", namespace.namespace_id.to_string()),
                ("base_ref", source.base_ref.clone()),
                ("base_commit", source.base_commit.clone()),
            ])
            .header(reqwest::header::CACHE_CONTROL, "no-cache, no-store")
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| AppError::Unavailable("forge_source_reader_unavailable".into()))?;
        if response.status() != reqwest::StatusCode::OK
            || response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
            || response.content_length().is_some_and(|n| n > 65_536)
        {
            return Err(AppError::Unavailable("forge_source_not_verified".into()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AppError::Unavailable("forge_source_reader_unavailable".into()))?
        {
            if bytes.len().saturating_add(chunk.len()) > 65_536 {
                return Err(AppError::Unavailable(
                    "forge_source_invalid_response".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        decode_source(&bytes, self.instance, namespace, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn old_catalog_metadata_is_not_source_proof() {
        let namespace = NamespaceRef {
            registry_instance_id: Uuid::new_v4(),
            namespace_id: Uuid::new_v4(),
        };
        let spec = domain::sdlc_source::SourcePolicySpec {
            forge_instance_id: Uuid::new_v4(),
            repository_id: Uuid::new_v4(),
            base_ref: "refs/heads/main".into(),
            base_commit: "a".repeat(40),
        };
        let body = serde_json::to_vec(
            &json!({"namespace":namespace,"forge_instance_id":spec.forge_instance_id,
            "repository_id":spec.repository_id,"public_name":"fixture/repo","kind":"hosted"}),
        )
        .unwrap();
        assert!(decode_source(&body, spec.forge_instance_id, &namespace, &spec).is_err());
    }

    #[test]
    fn exact_source_requires_namespace_instance_native_sha_and_fresh_time() {
        let namespace = NamespaceRef {
            registry_instance_id: Uuid::new_v4(),
            namespace_id: Uuid::new_v4(),
        };
        let spec = domain::sdlc_source::SourcePolicySpec {
            forge_instance_id: Uuid::new_v4(),
            repository_id: Uuid::new_v4(),
            base_ref: "refs/heads/main".into(),
            base_commit: "a".repeat(40),
        };
        let raw = json!({"namespace":namespace,"forge_instance_id":spec.forge_instance_id,"repository_id":spec.repository_id,
            "public_name":"fixture/repo","base_ref":spec.base_ref,"base_commit":spec.base_commit,
            "default_ref":"refs/heads/main","protected_refs":["refs/heads/main"],"observed_at":chrono::Utc::now()});
        let body = serde_json::to_vec(&raw).unwrap();
        assert!(decode_source(&body, spec.forge_instance_id, &namespace, &spec).is_ok());
        for field in [
            "namespace",
            "forge_instance_id",
            "repository_id",
            "base_ref",
            "base_commit",
            "observed_at",
        ] {
            let mut changed = raw.clone();
            changed[field] = match field {
                "namespace" => {
                    json!({"registry_instance_id":Uuid::new_v4(),"namespace_id":Uuid::new_v4()})
                }
                "base_ref" => json!("refs/heads/other"),
                "base_commit" => json!("f".repeat(40)),
                "observed_at" => json!(chrono::Utc::now() - chrono::Duration::seconds(31)),
                _ => json!(Uuid::new_v4()),
            };
            assert!(
                decode_source(
                    &serde_json::to_vec(&changed).unwrap(),
                    spec.forge_instance_id,
                    &namespace,
                    &spec
                )
                .is_err(),
                "{field}"
            );
        }
    }
}
