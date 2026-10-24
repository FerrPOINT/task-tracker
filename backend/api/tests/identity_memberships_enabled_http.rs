//! Separate process: enabled configuration precedes the only async runtime.
#[path = "support/identity_memberships_http.rs"]
mod fixture;
use axum::http::StatusCode;
use domain::identity_memberships::IdentityMembershipPage;
use sea_orm::{ConnectionTrait, TransactionTrait};
use std::{sync::atomic::Ordering, time::Duration};

struct Abort(tokio::task::AbortHandle);
impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
fn memberships_fresh_human_paging_and_machine_boundaries_never_jit() {
    fixture::run(true, |h| async move {
        let before = h.fingerprints().await;
        let tx = h.blocker.begin().await.unwrap();
        tx.execute_unprepared("LOCK TABLE users IN ACCESS EXCLUSIVE MODE")
            .await
            .unwrap();
        h.denied(None, "", StatusCode::UNAUTHORIZED).await;
        h.denied(Some("not-a-signed-token"), "", StatusCode::UNAUTHORIZED)
            .await;
        for name in [
            "human-pat",
            "machine-pat",
            "machine",
            "sessionless",
            "user",
            "namespace-machine",
            "reader-machine",
            "management-machine",
        ] {
            h.denied(Some(&h.tokens[name]), "", StatusCode::FORBIDDEN)
                .await;
        }
        // A non-admin browser needs this product in its enforced app grant.
        // The SDK's canonical global-admin exemption is tested positively below.
        // This route maps SDK Forbidden to Unauthorized, never a fake empty page.
        h.denied(Some(&h.tokens["foreign-app"]), "", StatusCode::UNAUTHORIZED)
            .await;
        h.denied(
            Some(&h.tokens["missing-proof"]),
            "",
            StatusCode::SERVICE_UNAVAILABLE,
        )
        .await;
        h.denied(Some(&h.tokens["disabled"]), "", StatusCode::UNAUTHORIZED)
            .await;
        let calls = h.auth_calls.load(Ordering::SeqCst);
        for query in ["?limit=0", "?limit=101", "?offset=18446744073709551615"] {
            h.denied(Some(&h.tokens["admin"]), query, StatusCode::BAD_REQUEST)
                .await;
        }
        h.denied(
            Some(&h.tokens["admin"]),
            "?unexpected=1",
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_eq!(
            h.auth_calls.load(Ordering::SeqCst),
            calls,
            "invalid bounded query must fail before central Auth and PG"
        );
        // Positive control: this exact relation barrier must block an admitted
        // real repository request. Denials above completed while it was held.
        let blocker_pid: i32 = tx
            .query_one(fixture::sql("SELECT pg_backend_pid() AS pid", []))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "pid")
            .unwrap();
        let request = h
            .client
            .get(format!(
                "{}/api/v2/identity-users/{}/memberships",
                h.origin, h.target
            ))
            .bearer_auth(&h.tokens["admin"]);
        let pending =
            tokio::spawn(async move { request.send().await.expect("admitted metadata request") });
        let _pending_guard = Abort(pending.abort_handle());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let waiting: bool = h
                    .observer
                    .query_one(fixture::sql(
                        "SELECT $1=ANY(pg_blocking_pids($2)) AS waiting",
                        [blocker_pid.into(), h.repository_pid.into()],
                    ))
                    .await
                    .unwrap()
                    .unwrap()
                    .try_get("", "waiting")
                    .unwrap();
                if waiting {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("admitted repository PID must reach the actual users relation barrier");
        assert!(!pending.is_finished());
        tx.rollback().await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(5), pending)
            .await
            .expect("admitted request completes after releasing only its barrier")
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.status(), StatusCode::OK);
        let page: IdentityMembershipPage = response.json().await.unwrap();
        assert_eq!((page.total, page.limit, page.offset), (2, 25, 0));
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.project_id)
                .collect::<Vec<_>>(),
            vec![h.member, h.owned]
        );
        assert_eq!(page.items[0].role, "viewer");
        assert_eq!(page.items[0].joined_at.unwrap().timestamp(), 1767323045);
        assert_eq!(page.items[0].spaces.len(), 1);
        assert_eq!(page.items[0].spaces[0].registry_instance_id, h.registry);
        assert_eq!(page.items[0].spaces[0].space_id, h.space);
        assert_eq!(page.items[1].role, "owner");
        assert!(page.items[1].joined_at.is_none());
        let response = h.get(Some(&h.tokens["global-admin"]), "?limit=1").await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "canonical global admin is admitted independently of the application list"
        );
        let global: IdentityMembershipPage = response.json().await.unwrap();
        assert_eq!(
            (
                global.total,
                global.limit,
                global.offset,
                global.items.len()
            ),
            (2, 1, 0, 1)
        );
        assert_eq!(global.items[0].project_id, h.member);
        let response = h.get(Some(&h.tokens["admin"]), "?limit=100").await;
        assert_eq!(response.status(), StatusCode::OK);
        let maximum: IdentityMembershipPage = response.json().await.unwrap();
        assert_eq!(
            (
                maximum.total,
                maximum.limit,
                maximum.offset,
                maximum.items.len()
            ),
            (2, 100, 0, 2)
        );
        for (query, offset, expected) in [
            ("?limit=1&offset=0", 0, Some(h.member)),
            ("?limit=1&offset=1", 1, Some(h.owned)),
            ("?limit=1&offset=2", 2, None),
        ] {
            let response = h.get(Some(&h.tokens["admin"]), query).await;
            assert_eq!(response.status(), StatusCode::OK);
            let page: IdentityMembershipPage = response.json().await.unwrap();
            assert_eq!((page.total, page.limit, page.offset), (2, 1, offset));
            assert_eq!(page.items.first().map(|item| item.project_id), expected);
            assert_eq!(page.items.len(), usize::from(expected.is_some()));
        }
        assert_eq!(
            h.fingerprints().await,
            before,
            "admitted metadata read must not create actor/target profiles or mutate memberships"
        );

        // A fresh, long-lived Auth application decision must never renew the
        // original signed JWT while its real SQL query is blocked.
        let tx = h.blocker.begin().await.unwrap();
        tx.execute_unprepared("LOCK TABLE users IN ACCESS EXCLUSIVE MODE")
            .await
            .unwrap();
        let blocker_pid: i32 = tx
            .query_one(fixture::sql("SELECT pg_backend_pid() AS pid", []))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "pid")
            .unwrap();
        let expires = chrono::Utc::now().timestamp() + 5;
        let token = h.short_admin(expires);
        let calls = h.auth_calls.load(Ordering::SeqCst);
        let request = h
            .client
            .get(format!(
                "{}/api/v2/identity-users/{}/memberships",
                h.origin, h.target
            ))
            .bearer_auth(token);
        let pending =
            tokio::spawn(
                async move { request.send().await.expect("short original proof request") },
            );
        let _expiry_guard = Abort(pending.abort_handle());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let waiting: bool = h
                    .observer
                    .query_one(fixture::sql(
                        "SELECT $1=ANY(pg_blocking_pids($2)) AS waiting",
                        [blocker_pid.into(), h.repository_pid.into()],
                    ))
                    .await
                    .unwrap()
                    .unwrap()
                    .try_get("", "waiting")
                    .unwrap();
                if waiting {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("short proof request must actually enter the exact repository barrier");
        let db_current: bool = h
            .observer
            .query_one(fixture::sql(
                "SELECT clock_timestamp()<to_timestamp(($1::bigint)::double precision) AS current",
                [expires.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "current")
            .unwrap();
        assert!(
            db_current && chrono::Utc::now().timestamp() < expires,
            "original JWT must still be current at the actual database barrier"
        );
        assert_eq!(h.auth_calls.load(Ordering::SeqCst), calls + 1);
        assert!(!pending.is_finished());
        tokio::time::timeout(Duration::from_secs(8),async {
            loop {
                let expired:bool=h.observer.query_one(fixture::sql("SELECT clock_timestamp()>=to_timestamp(($1::bigint)::double precision) AS expired",[expires.into()])).await.unwrap().unwrap().try_get("","expired").unwrap();
                if expired && chrono::Utc::now().timestamp()>=expires {break;}
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.expect("original signed deadline must expire under the held relation barrier");
        assert!(!pending.is_finished());
        tx.rollback().await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(5), pending)
            .await
            .expect("expired proof request completes after its SQL barrier releases")
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let error: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error["error"]["code"], "UNAVAILABLE");
        assert_eq!(
            error["error"]["message"],
            "identity_membership_authority_expired"
        );
        for field in ["items", "total", "limit", "offset"] {
            assert!(error.get(field).is_none());
        }
        assert_eq!(
            h.auth_calls.load(Ordering::SeqCst),
            calls + 1,
            "no fresh Auth request may renew the original proof after database I/O"
        );
        let retry = h.get(Some(&h.tokens["admin"]), "").await;
        assert_eq!(
            retry.status(),
            StatusCode::OK,
            "a new independently fresh request remains usable after the expired read"
        );
        let retry: IdentityMembershipPage = retry.json().await.unwrap();
        assert_eq!((retry.total, retry.items.len()), (2, 2));
        assert_eq!(
            h.fingerprints().await,
            before,
            "expired read must not mutate the seeded metadata"
        );
    });
}
