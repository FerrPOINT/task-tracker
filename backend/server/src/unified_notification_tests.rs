//! Actual digest cycle + migrated PostgreSQL + bounded SDK HTTP. The email port
//! captures the application delivery boundary; SMTP transport is unchanged and
//! is not claimed as exercised by this fixture. No real accounts/data/credentials.
use super::*;
use axum::{
    Json, Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use shared::{BoardId, ProjectId, ProjectKey, UserId, space_access::*};
use std::sync::{
    Mutex, RwLock,
    atomic::{AtomicBool, AtomicI64, Ordering},
};
use uuid::Uuid;
#[derive(Default)]
struct CapturedMail(Mutex<Vec<domain::EmailNotification>>);
#[async_trait::async_trait]
impl domain::EmailPort for CapturedMail {
    fn is_enabled(&self) -> bool {
        true
    }
    async fn send(&self, message: &domain::EmailNotification) -> Result<(), shared::AppError> {
        self.0.lock().unwrap().push(message.clone());
        Ok(())
    }
}
struct ProtocolState {
    target: Uuid,
    issuer: String,
    spaces: Vec<SpaceRef>,
    active: AtomicBool,
    selected: RwLock<Vec<SpaceRef>>,
    until: AtomicI64,
    revision: AtomicI64,
}
fn authorized(headers: &HeaderMap) -> bool {
    headers.get("authorization").and_then(|h| h.to_str().ok())
        == Some("Bearer sdlc_pat_synthetic-digest-reader")
}
fn validity(until: i64) -> AdmissionValidity {
    AdmissionValidity::confirmed(chrono_now(), until, i64::MAX).unwrap()
}
fn chrono_now() -> i64 {
    shared::now().timestamp()
}
async fn notification(
    ctx: &AppContext,
    user: UserId,
    project: ProjectId,
    title: &str,
) -> shared::NotificationId {
    ctx.repos
        .notifications
        .save(&domain::Notification {
            id: shared::NotificationId::new(),
            recipient_id: user,
            event_type: "test.project.event".into(),
            entity_type: "project".into(),
            entity_id: Some(project.as_uuid()),
            actor_id: None,
            title: title.into(),
            body: Some(title.into()),
            is_read: false,
            read_at: None,
            action_url: None,
            metadata: serde_json::json!({}),
            created_at: shared::now(),
        })
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_space_test PostgreSQL; invokes actual digest with protocol fixtures"]
async fn digest_delivery_preserves_denied_messages_and_rechecks_recipient_before_email() {
    let original =
        std::env::var("TT_UNIFIED_SPACE_TEST_DATABASE_URL").expect("owned test PostgreSQL");
    let root = Database::connect(&original).await.unwrap();
    let name = root
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "name")
        .unwrap();
    assert_eq!(name, "unified_pdlc4_space_test", "refuse working database");
    root.execute_unprepared("CREATE DATABASE notification_pdlc4_space_test")
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&original).unwrap();
    url.set_path("/notification_pdlc4_space_test");
    let db = Database::connect(url.to_string()).await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    let repo_db = Database::connect(url.to_string()).await.unwrap();
    let repo_pool = repo_db.get_postgres_connection_pool().clone();
    let repos = Arc::new(infra::repos::to_domain_repositories(
        infra::repos::SeaOrmRepositories::new(repo_db),
    ));
    let target = Uuid::new_v4();
    let user = repos
        .users
        .find_or_create_central_user(&target.to_string(), "recipient@example.test", "Recipient")
        .await
        .unwrap();
    assert!(!user.is_system_admin);
    let mut legacy = user.clone();
    legacy.id = UserId::new();
    legacy.email = "legacy@example.test".into();
    legacy.username = "legacy-profile".into();
    legacy.is_system_admin = true;
    repos.users.save(&legacy).await.unwrap();
    assert!(
        repos
            .users
            .central_subject(legacy.id)
            .await
            .unwrap()
            .is_none()
    );
    let mut config = AppConfig::default();
    config.auth.jwt_secret = "synthetic-unified-digest-test".into();
    let mail = Arc::new(CapturedMail::default());
    let ctx = AppContext::with_events(
        Arc::new(config),
        repos,
        Arc::new(domain::InMemoryStorage::default()),
        app::context::EventBus::default(),
        mail.clone(),
    );
    let mut projects = Vec::new();
    for key in ["READAB", "READB"] {
        let p = domain::Project {
            id: ProjectId::new(),
            key: ProjectKey::new(key),
            name: key.into(),
            description: None,
            owner_id: user.id,
            default_board_id: BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let status = ctx.repos.statuses.get_default().await.unwrap();
        let board = domain::Board {
            id: p.default_board_id,
            project_id: p.id,
            name: "Board".into(),
            columns: vec![domain::BoardColumn {
                id: status.id,
                name: status.name,
                category: status.category,
                wip_limit: None,
                position: 0,
            }],
        };
        ctx.repos
            .projects
            .save_with_board(&p, &board)
            .await
            .unwrap();
        projects.push(p.id);
    }
    let a = SpaceRef {
        registry_instance_id: Uuid::new_v4(),
        space_id: Uuid::new_v4(),
    };
    let b = SpaceRef {
        space_id: Uuid::new_v4(),
        ..a.clone()
    };
    for space in [&a, &b] {
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state) VALUES($1,$2,1,'active')",[space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
    }
    for (project, space) in [(projects[0], &a), (projects[0], &b), (projects[1], &b)] {
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",[project.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
    }
    db.execute_unprepared("UPDATE tracker_space_admission_state SET active=true")
        .await
        .unwrap();
    for recipient in [user.id, legacy.id] {
        ctx.repos
            .notification_settings
            .save_settings(&domain::NotificationUserSettings {
                user_id: recipient,
                email_frequency: "hourly".into(),
                disabled_event_types: vec![],
                notify_own_changes: true,
                last_email_digest_at: None,
            })
            .await
            .unwrap();
    }
    notification(&ctx, user.id, projects[0], "allowed A message").await;
    let denied = notification(&ctx, user.id, projects[1], "private B text").await;
    let legacy_message =
        notification(&ctx, legacy.id, projects[0], "legacy role is not admission").await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new(ProtocolState {
        target,
        issuer: origin.clone(),
        spaces: vec![a.clone(), b.clone()],
        active: AtomicBool::new(true),
        selected: RwLock::new(vec![a.clone()]),
        until: AtomicI64::new(chrono_now() + 3),
        revision: AtomicI64::new(1),
    });
    let profile_state = state.clone();
    let identity_state = state.clone();
    let admission_state = state.clone();
    let router=Router::new().route("/oidc/jwks",get(||async{Json(serde_json::json!({"keys":[]}))})).route("/auth/tokens/introspect",get(move |headers:HeaderMap|{let _state=profile_state.clone();async move{assert!(authorized(&headers));Json(serde_json::json!({"sub":"notification-machine","email":"machine@example.test","scopes":["admin-panel:read"],"subject_role":"service_account","observed_at":shared::now(),"expires_at":shared::now()+std::time::Duration::from_secs(300)}))}}))
    .route("/auth/identities/{id}",get(move |headers:HeaderMap,Path(id):Path<Uuid>|{let state=identity_state.clone();async move{assert!(authorized(&headers));if id!=state.target{return Err(StatusCode::NOT_FOUND);}Ok(Json(IdentityConfirmation{identity:CentralIdentity{issuer:state.issuer.clone(),subject:id.to_string()},active:state.active.load(Ordering::SeqCst),validity:validity(chrono_now()+60)}))}}))
    .route("/admission",post(move |headers:HeaderMap,Json(request):Json<SpaceAdmissionRequest>|{let state=admission_state.clone();async move{assert!(authorized(&headers));assert_eq!(request.identity.subject,state.target.to_string(),"processor privileges must never replace recipient admission");Json(SpaceAdmission{schema_version:1,identity:request.identity,identity_active:true,policy_revision:state.revision.load(Ordering::SeqCst),policy:SpaceAccessPolicy::Selected{spaces:state.selected.read().unwrap().clone()},validity:validity(state.until.load(Ordering::SeqCst)),spaces:state.spaces.iter().filter(|s|request.spaces.contains(s)).map(|s|ConfirmedSpace{space:s.clone(),revision:1,state:SpaceState::Active}).collect()})}}));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    // A dedicated ignored native process owns these synthetic variables/OnceCells.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{origin}/oidc/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &origin);
        std::env::set_var("TT_SPACES__ADMISSION_URL", format!("{origin}/admission"));
        std::env::set_var(
            "TT_SPACES__ADMISSION_TOKEN",
            "sdlc_pat_synthetic-digest-reader",
        );
        std::env::set_var(
            "TT_SPACES__ADMISSION_READER_SUBJECTS",
            "notification-machine",
        );
    }
    run_digest_cycle(&ctx, shared::now()).await.unwrap();
    let sent = mail.0.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].body.contains("allowed A message"));
    assert!(!sent[0].body.contains("private B"));
    let unread = ctx.repos.notifications.list_all_unread().await.unwrap();
    assert!(unread.iter().any(|n| n.id == denied));
    assert!(unread.iter().any(|n| n.id == legacy_message));
    notification(&ctx, user.id, projects[0], "new A after disable").await;
    state.active.store(false, Ordering::SeqCst);
    run_digest_cycle(&ctx, shared::now()).await.unwrap();
    assert_eq!(
        mail.0.lock().unwrap().len(),
        1,
        "fresh disabled identity wins over cached positive admission"
    );
    state.active.store(true, Ordering::SeqCst);
    *state.selected.write().unwrap() = vec![];
    state.revision.store(2, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(3100)).await;
    state.until.store(chrono_now() + 60, Ordering::SeqCst);
    run_digest_cycle(&ctx, shared::now()).await.unwrap();
    assert_eq!(mail.0.lock().unwrap().len(), 1);
    assert_eq!(
        ctx.repos.notifications.count_unread(user.id).await.unwrap(),
        2,
        "denied historical messages remain unread, not deleted"
    );
    server.abort();
    drop(ctx);
    repo_pool.close().await;
    db.close().await.unwrap();
    root.execute_unprepared("DROP DATABASE notification_pdlc4_space_test")
        .await
        .unwrap();
    root.close().await.unwrap();
}
