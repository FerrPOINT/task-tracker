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
                "api/v1/namespace-repositories/{}/source",
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

    #[tokio::test]
    async fn source_request_uses_dedicated_endpoint_and_existing_reader_credential() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let namespace = NamespaceRef {
            registry_instance_id: Uuid::new_v4(),
            namespace_id: Uuid::new_v4(),
        };
        let source = domain::sdlc_source::SourcePolicySpec {
            forge_instance_id: Uuid::new_v4(),
            repository_id: Uuid::new_v4(),
            base_ref: "refs/heads/main".into(),
            base_commit: "a".repeat(40),
        };
        let proof = json!({"namespace":namespace,"forge_instance_id":source.forge_instance_id,
            "repository_id":source.repository_id,"public_name":"fixture/repository",
            "base_ref":source.base_ref,"base_commit":source.base_commit,
            "default_ref":"refs/heads/main","protected_refs":["refs/heads/main"],
            "observed_at":chrono::Utc::now()});
        let expected = format!(
            "/api/v1/namespace-repositories/{}/source",
            source.repository_id
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint =
            reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let expected_path = expected.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                let mut buffer = [0; 2048];
                let count = stream.read(&mut buffer).await.unwrap();
                assert!(count > 0 && bytes.len() + count <= 8192);
                bytes.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(bytes).unwrap();
            let first: Vec<_> = request.lines().next().unwrap().split_whitespace().collect();
            assert_eq!(first[0], "GET");
            let target = reqwest::Url::parse(&format!("http://fixture.test{}", first[1])).unwrap();
            let authorization = request
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                .map(|(_, value)| value.trim().to_owned())
                .unwrap();
            // The old catalog route never supplies verification, even with source query fields.
            let body = if target.path() == expected_path {
                proof
            } else {
                json!({"kind":"hosted","public_name":"fixture/repository"})
            };
            let body = serde_json::to_vec(&body).unwrap();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
            (target, authorization)
        });
        let reader = ForgeReader {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            endpoint,
            token: "fixture-reader-credential".into(),
            instance: source.forge_instance_id,
        };
        let observed = reader.verify_source_observation(&namespace, &source).await;
        let (target, authorization) = server.await.unwrap();
        assert_eq!(target.path(), expected);
        assert_eq!(authorization, "Bearer fixture-reader-credential");
        let query: std::collections::HashMap<_, _> = target.query_pairs().into_owned().collect();
        assert_eq!(query.len(), 4);
        assert_eq!(
            query["registry_instance_id"],
            namespace.registry_instance_id.to_string()
        );
        assert_eq!(query["namespace_id"], namespace.namespace_id.to_string());
        assert_eq!(query["base_ref"], source.base_ref);
        assert_eq!(query["base_commit"], source.base_commit);
        assert_eq!(observed.unwrap().repository_id, source.repository_id);
    }

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
