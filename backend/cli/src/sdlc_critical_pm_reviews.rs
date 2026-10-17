use super::readable_key;
use crate::commands::{json_input, sdlc_path};
use crate::{Api, enc};
use anyhow::Result;
use clap::Subcommand;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum CriticalPmReviewCommand {
    /// Scheduler WRITE only: reserve a new PM review, not a native dispatch or initial Draft
    Reserve {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Scheduler READ only: read the original operation after an unknown acknowledgement
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

// The CLI intentionally has no production domain dependency. Agreement is checked below.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservationInput {
    blocker_id: Uuid,
    blocker_sha256: String,
    expected_blocker_version: i64,
    expected_task_version: i64,
    expected_pm_owner_version: i64,
    expected_pm_assignment_version: i64,
    idempotency_key: String,
}

fn validate_input(task: Uuid, payload: &Value) -> Result<()> {
    let input: ReservationInput = serde_json::from_value(payload.clone())
        .map_err(|_| anyhow::anyhow!("invalid critical PM review reservation payload"))?;
    anyhow::ensure!(
        !task.is_nil()
            && !input.blocker_id.is_nil()
            && input.blocker_sha256.len() == 64
            && input
                .blocker_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && [
                input.expected_blocker_version,
                input.expected_task_version,
                input.expected_pm_owner_version,
                input.expected_pm_assignment_version
            ]
            .iter()
            .all(|version| (1..=9_007_199_254_740_991).contains(version))
            && readable_key(&input.idempotency_key),
        "invalid critical PM review reservation payload"
    );
    Ok(())
}

pub async fn pm_review(api: &Api, command: CriticalPmReviewCommand) -> Result<Value> {
    match command {
        CriticalPmReviewCommand::Reserve { task, from_file } => {
            let payload = json_input(&from_file)?;
            validate_input(task, &payload)?;
            api.post(&sdlc_path(task, "/critical-blockers/pm-reviews"), payload)
                .await
        }
        CriticalPmReviewCommand::Operation { task, key } => {
            anyhow::ensure!(
                !task.is_nil() && readable_key(&key),
                "invalid critical PM review task or key"
            );
            api.get(&sdlc_path(
                task,
                &format!("/critical-blockers/pm-reviews/operations/{}", enc(&key)),
            ))
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sdlc_critical_pm_reservation::ReserveCriticalPmReview;
    use serde_json::json;

    #[test]
    fn closed_cli_input_agrees_with_domain_and_preserves_original_body() {
        let task = Uuid::new_v4();
        let body = json!({"blocker_id":Uuid::new_v4(),"blocker_sha256":"a".repeat(64),
            "expected_blocker_version":2,"expected_task_version":9,"expected_pm_owner_version":17,
            "expected_pm_assignment_version":17,"idempotency_key":"pm-review:original?x=+%#"});
        let original = body.clone();
        validate_input(task, &body).unwrap();
        assert_eq!(body, original);
        for field in [
            "expected_blocker_version",
            "expected_task_version",
            "expected_pm_owner_version",
            "expected_pm_assignment_version",
        ] {
            for value in [
                json!(0),
                json!(-1),
                json!(null),
                json!(1.5),
                json!("1"),
                json!(9_007_199_254_740_992_i64),
                json!(9_007_199_254_740_991_i64),
            ] {
                let mut input = body.clone();
                input[field] = value;
                assert_eq!(
                    validate_input(task, &input).is_ok(),
                    serde_json::from_value::<ReserveCriticalPmReview>(input)
                        .is_ok_and(|v| v.validate().is_ok())
                );
            }
        }
        for (field, value) in [
            ("blocker_id", json!(Uuid::nil())),
            ("blocker_sha256", json!("A".repeat(64))),
            ("idempotency_key", json!("../bad")),
            ("requested_agent_id", json!(Uuid::new_v4())),
            ("dispatch_allowed", json!(true)),
        ] {
            let mut input = body.clone();
            input[field] = value;
            assert!(validate_input(task, &input).is_err());
            assert!(
                !serde_json::from_value::<ReserveCriticalPmReview>(input)
                    .is_ok_and(|v| v.validate().is_ok())
            );
        }
        assert!(validate_input(Uuid::nil(), &body).is_err());
    }
}
