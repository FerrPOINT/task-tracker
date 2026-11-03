//! Fast standalone MFA service regressions; SQL consumption races have a separate PostgreSQL gate.
use app::totp::TotpService;
use domain::UserRepository;
use std::sync::Arc;
async fn fixture() -> (TotpService, Arc<domain::Repositories>, shared::UserId) {
    let users = Arc::new(domain::MemoryUserRepository::default());
    let id = shared::UserId::new();
    let now = shared::now();
    users
        .save(&domain::User {
            id,
            email: "mfa@example.test".into(),
            username: "mfa".into(),
            display_name: "MFA".into(),
            password_hash: "!".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
    let repos = Arc::new(domain::Repositories {
        users,
        totp: Arc::new(domain::stubs::memory::MemoryTotpRepository::default()),
        ..Default::default()
    });
    (
        TotpService::new(repos.clone(), Arc::new(shared::AppConfig::default().auth)),
        repos,
        id,
    )
}
fn step() -> i64 {
    chrono::Utc::now().timestamp() / 30
}
#[tokio::test]
async fn future_window_code_is_recorded_as_its_actual_step_and_cannot_be_replayed() {
    let (service, repos, user) = fixture().await;
    let enrollment = service.setup(user).await.unwrap();
    let current = step();
    let code = TotpService::code_for_step(&enrollment.secret, current).unwrap();
    service.enable(user, &code).await.unwrap();
    let future = current + 1;
    let code = TotpService::code_for_step(&enrollment.secret, future).unwrap();
    assert!(service.login_verify(user, &code).await.unwrap());
    assert_eq!(
        repos.totp.get(user).await.unwrap().last_used_step,
        future,
        "persist the matched step, including the accepted future window"
    );
    assert!(
        !service.login_verify(user, &code).await.unwrap(),
        "the same accepted code is single use"
    );
}
#[tokio::test]
async fn setup_cannot_reset_an_enabled_enrollment() {
    let (service, repos, user) = fixture().await;
    let enrollment = service.setup(user).await.unwrap();
    let code = TotpService::code_for_step(&enrollment.secret, step()).unwrap();
    let recovery = service.enable(user, &code).await.unwrap();
    assert_eq!(recovery.recovery_codes.len(), 8);
    let before = repos.totp.get(user).await.unwrap();
    assert!(
        matches!(
            service.setup(user).await,
            Err(shared::AppError::Conflict(_))
        ),
        "setting up another secret requires the existing confirmed disable flow"
    );
    let after = repos.totp.get(user).await.unwrap();
    assert!(after.enabled);
    assert_eq!(after.secret_cipher, before.secret_cipher);
    assert_eq!(after.recovery_codes, before.recovery_codes);
}
