use anyhow::{Result, bail};
use uuid::Uuid;

pub fn configured_token(
    protocol: Option<&str>,
    agent: Option<&str>,
    dedicated: Option<&str>,
    supplied: Option<&str>,
) -> Result<Option<String>> {
    if protocol != Some("configured-cli-v1") {
        return Ok(supplied.map(str::to_owned));
    }
    let agent = agent.ok_or_else(|| anyhow::anyhow!("FLEET_AGENT_ID is required"))?;
    let id = Uuid::parse_str(agent)?;
    if id.is_nil() || id.to_string() != agent {
        bail!("FLEET_AGENT_ID must be a canonical nonnil UUID");
    }
    let token = dedicated
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("TASKTRACKER_TOKEN is required in configured agent mode"))?;
    if supplied.is_some_and(|s| s.trim() != token.trim()) {
        bail!("configured agent mode requires its dedicated TASKTRACKER_TOKEN");
    }
    Ok(Some(token.trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_agent_never_uses_global_or_alternate_credentials() {
        let agent = "11111111-1111-4111-8111-111111111111";
        for token in [None, Some(""), Some(" ")] {
            assert!(
                configured_token(
                    Some("configured-cli-v1"),
                    Some(agent),
                    token,
                    Some("global")
                )
                .is_err()
            );
        }
        assert!(
            configured_token(Some("configured-cli-v1"), None, Some("dedicated"), None).is_err()
        );
        assert!(
            configured_token(
                Some("configured-cli-v1"),
                Some(agent),
                Some("dedicated"),
                Some("global")
            )
            .is_err()
        );
        assert_eq!(
            configured_token(
                Some("configured-cli-v1"),
                Some(agent),
                Some("dedicated"),
                None
            )
            .unwrap()
            .as_deref(),
            Some("dedicated")
        );
    }

    #[test]
    fn manual_legacy_token_selection_is_unchanged() {
        assert_eq!(
            configured_token(None, None, None, Some("manual"))
                .unwrap()
                .as_deref(),
            Some("manual")
        );
        assert!(configured_token(None, None, None, None).unwrap().is_none());
    }
}
