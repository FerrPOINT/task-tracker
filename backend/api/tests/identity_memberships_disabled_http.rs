//! Separate process: default-off configuration precedes the only async runtime.
#[path = "support/identity_memberships_http.rs"]
mod fixture;
use axum::http::StatusCode;
use sea_orm::{ConnectionTrait, TransactionTrait};
use std::sync::atomic::Ordering;

#[test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
fn memberships_default_off_does_not_query_or_provision() {
    fixture::run(false, |h| async move {
        let before = h.fingerprints().await;
        let tx = h.blocker.begin().await.unwrap();
        tx.execute_unprepared("LOCK TABLE users IN ACCESS EXCLUSIVE MODE")
            .await
            .unwrap();
        h.denied(
            Some(&h.tokens["admin"]),
            "",
            StatusCode::SERVICE_UNAVAILABLE,
        )
        .await;
        assert_eq!(
            h.auth_calls.load(Ordering::SeqCst),
            1,
            "disabled projection still verifies the supplied central identity"
        );
        h.denied(None, "", StatusCode::UNAUTHORIZED).await;
        assert_eq!(h.auth_calls.load(Ordering::SeqCst), 1);
        tx.rollback().await.unwrap();
        assert_eq!(
            h.fingerprints().await,
            before,
            "default-off must not JIT or mutate seeded metadata"
        );
    });
}
