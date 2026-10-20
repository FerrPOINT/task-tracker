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
    atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
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
    shadow_checks: AtomicUsize,
    shadow_initial_until: AtomicI64,
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
    for key in ["READAB", "READB", "OTHEROWNER"] {
        let p = domain::Project {
            id: ProjectId::new(),
            key: ProjectKey::new(key),
            name: key.into(),
            description: None,
            owner_id: if key == "OTHEROWNER" {
                legacy.id
            } else {
                user.id
            },
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
    for (project, space) in [
        (projects[0], &a),
        (projects[0], &b),
        (projects[1], &b),
        (projects[2], &a),
    ] {
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",[project.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
    }
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
        shadow_checks: AtomicUsize::new(0),
        shadow_initial_until: AtomicI64::new(0),
    });
    let profile_state = state.clone();
    let identity_state = state.clone();
    let admission_state = state.clone();
    let router = Router::new()
        .route("/oidc/jwks",get(||async{Json(serde_json::json!({"keys":[]}))}))
        .route("/auth/tokens/introspect",get(move |headers:HeaderMap|{
            let state=profile_state.clone();
            async move {
                let token=headers.get("authorization").and_then(|v|v.to_str().ok()).unwrap_or("");
                let (subject,email,role,scopes)=if authorized(&headers){
                    ("notification-machine".to_string(),"machine@example.test","service_account",vec!["admin-panel:read"])
                }else if token=="Bearer sdlc_pat_synthetic-shadow-read"{
                    if state.shadow_checks.fetch_add(1,Ordering::SeqCst)>0 {
                        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
                    }
                    (state.target.to_string(),"recipient@example.test","member",vec!["task-tracker:read"])
                }else if token=="Bearer sdlc_pat_synthetic-human-read"{
                    (state.target.to_string(),"recipient@example.test","member",vec!["task-tracker:read"])
                }else if token=="Bearer sdlc_pat_synthetic-human-write"{
                    (state.target.to_string(),"recipient@example.test","member",vec!["task-tracker:read","task-tracker:write"])
                }else{return (StatusCode::UNAUTHORIZED,Json(serde_json::json!({})))};
                let observed_at=shared::now();
                if token=="Bearer sdlc_pat_synthetic-shadow-read" {
                    let _=state.shadow_initial_until.compare_exchange(0,observed_at.timestamp()+60,Ordering::SeqCst,Ordering::SeqCst);
                }
                (StatusCode::OK,Json(serde_json::json!({"sub":subject,"email":email,"display_name":"Recipient",
                    "scopes":scopes,"subject_role":role,"observed_at":observed_at,
                    "expires_at":observed_at+std::time::Duration::from_secs(300)})))
            }
        }))
        .route("/auth/identities/{id}",get(move |headers:HeaderMap,Path(id):Path<Uuid>|{
            let state=identity_state.clone();
            async move{
                assert!(authorized(&headers));if id!=state.target{return Err(StatusCode::NOT_FOUND);}
                Ok(Json(IdentityConfirmation{identity:CentralIdentity{issuer:state.issuer.clone(),subject:id.to_string()},
                    active:state.active.load(Ordering::SeqCst),validity:validity(chrono_now()+60)}))
            }
        }))
        .route("/admission",post(move |headers:HeaderMap,Json(request):Json<SpaceAdmissionRequest>|{
            let state=admission_state.clone();
            async move{
                assert!(authorized(&headers));assert_eq!(request.identity.subject,state.target.to_string(),
                    "processor privileges must never replace the original identity");
                Json(SpaceAdmission{schema_version:1,identity:request.identity,identity_active:true,
                    policy_revision:state.revision.load(Ordering::SeqCst),
                    policy:SpaceAccessPolicy::Selected{spaces:state.selected.read().unwrap().clone()},
                    validity:validity(state.until.load(Ordering::SeqCst)),
                    spaces:state.spaces.iter().filter(|s|request.spaces.contains(s))
                        .map(|s|ConfirmedSpace{space:s.clone(),revision:1,state:SpaceState::Active}).collect()})
            }
        }));
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
    let ctx = Arc::new(ctx);
    let api_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api_origin = format!("http://{}", api_listener.local_addr().unwrap());
    let api_app = api::router(ctx.clone()).with_state(ctx.clone());
    let api_server = tokio::spawn(async move {
        axum::serve(
            api_listener,
            api_app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    // Staged V2 routes still preserve the middleware's original authorization
    // window even when they must fetch their own space proof.
    state.until.store(chrono_now() + 120, Ordering::SeqCst);
    let shadow = http
        .get(format!("{api_origin}/api/v2/projects/OTHEROWNER/spaces"))
        .bearer_auth("sdlc_pat_synthetic-shadow-read")
        .send()
        .await
        .unwrap();
    assert_eq!(shadow.status(), StatusCode::OK);
    assert!(
        state.shadow_checks.load(Ordering::SeqCst) >= 2,
        "exercise the staged route's second verification"
    );
    let shadow_until = shadow.headers()["x-pdlc-admission-valid-until"]
        .to_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let shadow_links: ConfirmedProjectSpaces = shadow.json().await.unwrap();
    let initial_until = state.shadow_initial_until.load(Ordering::SeqCst);
    assert!(initial_until > 0);
    assert!(
        shadow_until <= initial_until,
        "headers cannot extend the original middleware confirmation"
    );
    assert!(
        shadow_links.validity.valid_until <= initial_until,
        "a second verification must not renew the response body"
    );
    println!("Native staged V2 original-window preservation PASS");
    db.execute_unprepared("UPDATE tracker_space_admission_state SET active=true")
        .await
        .unwrap();
    state.until.store(chrono_now() + 3, Ordering::SeqCst);
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
    state.until.store(chrono_now() + 3, Ordering::SeqCst);
    run_digest_cycle(&ctx, shared::now()).await.unwrap();
    assert_eq!(mail.0.lock().unwrap().len(), 1);
    assert_eq!(
        ctx.repos.notifications.count_unread(user.id).await.unwrap(),
        2,
        "denied historical messages remain unread, not deleted"
    );
    tokio::time::sleep(std::time::Duration::from_millis(3100)).await;
    *state.selected.write().unwrap() = vec![a.clone()];
    state.revision.store(3, Ordering::SeqCst);
    state.until.store(chrono_now() + 30, Ordering::SeqCst);
    let read = http
        .get(format!("{api_origin}/api/v2/projects/READAB/spaces"))
        .bearer_auth("sdlc_pat_synthetic-human-read")
        .header("x-pdlc-project-spaces-can-change", "true")
        .header("x-pdlc-admission-policy-revision", "999999")
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        read.headers()["x-pdlc-project-spaces-can-change"],
        "false",
        "neither owner_id nor a supplied header grants write scope"
    );
    assert_eq!(
        read.headers()["x-pdlc-admission-policy-revision"],
        state.revision.load(Ordering::SeqCst).to_string(),
        "request headers never choose policy revision"
    );
    let header_until = read.headers()["x-pdlc-admission-valid-until"]
        .to_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let confirmed: ConfirmedProjectSpaces = read.json().await.unwrap();
    assert_eq!(confirmed.project.project_id, projects[0].as_uuid());
    assert_eq!(
        confirmed.spaces.len(),
        2,
        "the hidden B association remains, without reading B content"
    );
    assert!(confirmed.validity.valid_until <= state.until.load(Ordering::SeqCst));
    assert!(header_until <= state.until.load(Ordering::SeqCst));
    assert!(confirmed.validity.valid_until > chrono_now());
    let denied_change=http.put(format!("{api_origin}/api/v2/projects/READAB/spaces"))
        .bearer_auth("sdlc_pat_synthetic-human-read")
        .json(&serde_json::json!({"operation_id":Uuid::new_v4(),"expected_revision":confirmed.revision,"spaces":[a]}))
        .send().await.unwrap();
    assert_eq!(denied_change.status(), StatusCode::FORBIDDEN);
    drop(denied_change);
    let writer = http
        .get(format!("{api_origin}/api/v2/projects/READAB/spaces"))
        .bearer_auth("sdlc_pat_synthetic-human-write")
        .send()
        .await
        .unwrap();
    assert_eq!(writer.status(), StatusCode::OK);
    assert_eq!(writer.headers()["x-pdlc-project-spaces-can-change"], "true");
    let writer_links: ConfirmedProjectSpaces = writer.json().await.unwrap();
    assert_eq!(
        writer_links.spaces, confirmed.spaces,
        "rejected read PAT did not mutate associations"
    );
    let other = http
        .get(format!("{api_origin}/api/v2/projects/OTHEROWNER/spaces"))
        .bearer_auth("sdlc_pat_synthetic-human-write")
        .send()
        .await
        .unwrap();
    assert_eq!(other.status(), StatusCode::OK);
    assert_eq!(
        other.headers()["x-pdlc-project-spaces-can-change"],
        "false",
        "admission/read+write is not project ownership"
    );
    let _: ConfirmedProjectSpaces = other.json().await.unwrap();
    let committed_operation = Uuid::new_v4();
    let committed_change = serde_json::json!({"operation_id":committed_operation,"expected_revision":writer_links.revision,"spaces":writer_links.spaces});
    let committed = http
        .put(format!("{api_origin}/api/v2/projects/READAB/spaces"))
        .bearer_auth("sdlc_pat_synthetic-human-write")
        .json(&committed_change)
        .send()
        .await
        .unwrap();
    assert_eq!(committed.status(), StatusCode::OK);
    let _: ConfirmedProjectSpaces = committed.json().await.unwrap();
    let lifecycle_url = format!("{api_origin}/api/v2/projects/READAB/lifecycle");
    let lifecycle: shared::project_spaces::ProjectLifecycle = http
        .get(&lifecycle_url)
        .bearer_auth("sdlc_pat_synthetic-human-write")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let archived=http.put(&lifecycle_url).bearer_auth("sdlc_pat_synthetic-human-write")
        .json(&serde_json::json!({"operation_id":Uuid::new_v4(),"expected_revision":lifecycle.revision,"archived":true}))
        .send().await.unwrap();
    assert_eq!(archived.status(), StatusCode::OK);
    assert!(
        archived
            .json::<shared::project_spaces::ProjectLifecycle>()
            .await
            .unwrap()
            .archived
    );
    let readonly = http
        .get(format!("{api_origin}/api/v2/projects/READAB/spaces"))
        .bearer_auth("sdlc_pat_synthetic-human-write")
        .send()
        .await
        .unwrap();
    assert_eq!(readonly.status(), StatusCode::OK);
    assert_eq!(
        readonly.headers()["x-pdlc-project-spaces-can-change"],
        "false",
        "project archive independently closes changes"
    );
    let readonly_links: ConfirmedProjectSpaces = readonly.json().await.unwrap();
    assert_eq!(
        readonly_links.spaces, confirmed.spaces,
        "archive preserves associations"
    );
    let receipt = http
        .get(format!(
            "{api_origin}/api/v2/projects/READAB/spaces/operations/{committed_operation}"
        ))
        .bearer_auth("sdlc_pat_synthetic-human-read")
        .send()
        .await
        .unwrap();
    assert_eq!(
        receipt.status(),
        StatusCode::OK,
        "archive and read-only scope must not hide an authored committed receipt"
    );
    let receipt: domain::project_spaces::ProjectSpaceOperationReceipt =
        receipt.json().await.unwrap();
    assert!(receipt.valid());
    assert!(receipt.applied);
    assert_eq!(receipt.operation_id, committed_operation);
    assert_eq!(
        serde_json::to_value(&receipt.change).unwrap(),
        committed_change
    );
    assert!(receipt.validity.valid_until <= state.until.load(Ordering::SeqCst));
    let foreign_actor = CentralIdentity {
        issuer: origin.clone(),
        subject: Uuid::new_v4().to_string(),
    };
    assert!(
        ctx.repos
            .projects
            .project_space_operation(projects[0], committed_operation, &foreign_actor)
            .await
            .unwrap()
            .is_none()
    );
    let original_actor = CentralIdentity {
        issuer: origin.clone(),
        subject: target.to_string(),
    };
    assert!(
        ctx.repos
            .projects
            .project_space_operation(projects[2], committed_operation, &original_actor)
            .await
            .unwrap()
            .is_none()
    );
    let absent = http
        .get(format!(
            "{api_origin}/api/v2/projects/READAB/spaces/operations/{}",
            Uuid::new_v4()
        ))
        .bearer_auth("sdlc_pat_synthetic-human-read")
        .send()
        .await
        .unwrap();
    assert_eq!(absent.status(), StatusCode::NOT_FOUND);
    drop(absent);
    println!(
        "Native HTTP management/receipt: scope/owner/archive/author/project/original deadline PASS"
    );
    drop(http);
    api_server.abort();
    let _ = api_server.await;
    server.abort();
    drop(ctx);
    repo_pool.close().await;
    db.close().await.unwrap();
    root.execute_unprepared("DROP DATABASE notification_pdlc4_space_test")
        .await
        .unwrap();
    root.close().await.unwrap();
}
