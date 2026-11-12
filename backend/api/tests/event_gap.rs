use axum::{Extension, extract::State, response::IntoResponse};
use std::{sync::Arc, time::Duration};
use tokio_stream::StreamExt;
#[tokio::test]
async fn lag_is_explicit_and_contains_no_foreign_resource_ids() {
    let mut config = shared::AppConfig::default();
    config.auth.jwt_secret = "synthetic-gap-fixture-only".into();
    let ctx = Arc::new(app::AppContext::new(
        Arc::new(config),
        Arc::new(domain::Repositories::default()),
        Arc::new(domain::InMemoryStorage::default()),
    ));
    let user = shared::UserId::new();
    let foreign = shared::UserId::new();
    let claims = app::auth::UserClaims {
        sub: user.to_string(),
        exp: (chrono::Utc::now().timestamp() + 60) as usize,
        typ: Some("access".into()),
        jti: None,
    };
    let response = api::routes::events::events(State(ctx.clone()), Extension(claims))
        .await
        .unwrap()
        .into_response();
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    for _ in 0..300 {
        ctx.events
            .publish(shared::TrackerEvent::NotificationCreated {
                recipient_id: foreign.to_string(),
            });
    }
    let mut stream = response.into_body().into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(first.to_vec()).unwrap();
    assert_eq!(text, "event: resync_required\ndata: {}\n\n");
    assert!(!text.contains(&foreign.to_string()));
    ctx.events
        .publish(shared::TrackerEvent::NotificationCreated {
            recipient_id: user.to_string(),
        });
    let mut received = false;
    for _ in 0..3 {
        let next = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = String::from_utf8(next.to_vec()).unwrap();
        assert!(!text.contains(&foreign.to_string()));
        if text.contains(&user.to_string()) {
            assert!(text.contains("notification_created"));
            received = true;
            break;
        }
        assert_eq!(text, "event: resync_required\ndata: {}\n\n");
    }
    assert!(received, "authorized changes must resume after the gap");
}
