//! Explicit SDK HTTP fixture: space policy remains independent of app policy.
//! No application database, runtime credentials or real users are involved.
use axum::{
    Json, Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde_json::json;
use shared::space_access::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use uuid::Uuid;
const READER: &str = "pdlc_pat_fixture_space_reader_AAAAAAAAAAAAAAAAA";
const NATIVE: &str = "pdlc_pat_fixture_native_writer_AAAAAAAAAAAAAAAA";
#[tokio::test]
#[ignore = "explicit isolated owner HTTP qualification; changes process environment"]
async fn space_only_native_owner_read_requires_registered_reader_and_current_identity() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let owner = Uuid::new_v4();
    let native_subject = Uuid::new_v4().to_string();
    let space = SpaceRef {
        registry_instance_id: Uuid::new_v4(),
        space_id: Uuid::new_v4(),
    };
    let active = Arc::new(AtomicBool::new(true));
    let identity_reads = Arc::new(AtomicUsize::new(0));
    let policy_reads = Arc::new(AtomicUsize::new(0));
    let native_id = native_subject.clone();
    let identity_issuer = issuer.clone();
    let identity_active = active.clone();
    let identities = identity_reads.clone();
    let spaces = vec![space.clone()];
    let admissions = policy_reads.clone();
    let stub=Router::new()
 .route("/jwks",get(||async{Json(json!({"keys":[]}))}))
 .route("/auth/tokens/introspect",get(move|headers:HeaderMap|{let native_id=native_id.clone();async move{
  let token=headers.get("authorization").unwrap().to_str().unwrap();
  let (subject,scopes)=if token==format!("Bearer {READER}"){("space-reader-qa".to_owned(),vec!["admin-panel:read"])}else if token==format!("Bearer {NATIVE}"){(native_id,vec!["task-tracker:write"])}else{return (StatusCode::UNAUTHORIZED,Json(json!({})))};
  let now=chrono::Utc::now();(StatusCode::OK,Json(json!({"sub":subject,"email":"machine@example.test","display_name":"Fixture","subject_role":"service_account","scopes":scopes,"observed_at":now,"expires_at":now+chrono::Duration::seconds(60)})))
 }}))
 .route("/auth/identities/{id}",get(move|headers:HeaderMap,Path(id):Path<Uuid>|{let issuer=identity_issuer.clone();let active=identity_active.clone();let reads=identities.clone();async move{
  assert_eq!(headers.get("authorization").unwrap().to_str().unwrap(),format!("Bearer {READER}"));assert_eq!(id,owner);reads.fetch_add(1,Ordering::SeqCst);
  let now=chrono::Utc::now().timestamp();Json(IdentityConfirmation{identity:CentralIdentity{issuer,subject:id.to_string()},active:active.load(Ordering::SeqCst),validity:AdmissionValidity::confirmed(now,now+60,i64::MAX).unwrap()})
 }}))
 .route("/admission",post(move|headers:HeaderMap,Json(query):Json<SpaceAdmissionRequest>|{let spaces=spaces.clone();let reads=admissions.clone();async move{
  assert_eq!(headers.get("authorization").unwrap().to_str().unwrap(),format!("Bearer {READER}"));assert_eq!(query.identity.subject,owner.to_string(),"service identity never replaces the human owner");reads.fetch_add(1,Ordering::SeqCst);
  let now=chrono::Utc::now().timestamp();Json(SpaceAdmission{schema_version:1,identity:query.identity,identity_active:true,policy_revision:1,policy:SpaceAccessPolicy::Selected{spaces:spaces.clone()},validity:AdmissionValidity::confirmed(now,now+30,i64::MAX).unwrap(),spaces:spaces.into_iter().map(|space|ConfirmedSpace{space,revision:1,state:SpaceState::Active}).collect(),inheritance:None})
 }}));
    let server = tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED", "false");
        std::env::remove_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN");
        std::env::set_var("TT_SPACES__ADMISSION_URL", format!("{issuer}/admission"));
        std::env::set_var("TT_SPACES__ADMISSION_TOKEN", READER);
        std::env::remove_var("TT_SPACES__ADMISSION_READER_SUBJECTS");
    }
    assert!(
        !app::native_space_access::application_policy_required()
            .await
            .unwrap()
    );
    let verified = app::space_access::CENTRAL_BRIDGE
        .try_verified_token(NATIVE)
        .await
        .unwrap();
    let native = app::native_space_access::NativeRequest::verified(&verified, true, true).unwrap();
    assert_eq!(native.identity().subject, native_subject);
    assert!(
        matches!(
            native.owner_space_read(owner, &[space.clone()]).await,
            Err(shared::AppError::Unavailable(_))
        ),
        "missing reader registration must not fall back to the native caller"
    );
    unsafe {
        std::env::set_var("TT_SPACES__ADMISSION_READER_SUBJECTS", "foreign-reader");
    }
    assert!(matches!(
        native.owner_space_read(owner, &[space.clone()]).await,
        Err(shared::AppError::Forbidden)
    ));
    assert_eq!(identity_reads.load(Ordering::SeqCst), 0);
    unsafe {
        std::env::set_var("TT_SPACES__ADMISSION_READER_SUBJECTS", "space-reader-qa");
    }
    assert!(
        native.confirmed_owner(owner).is_err(),
        "locked code cannot invent an unresolved owner proof"
    );
    native.require_known_owner(owner).await.unwrap();
    native.confirmed_owner(owner).unwrap();
    assert!(
        native.require_known_owner(Uuid::new_v4()).await.is_err(),
        "one request cannot switch its immutable owner"
    );
    let identity_baseline = identity_reads.load(Ordering::SeqCst);
    let proof = native
        .owner_space_read(owner, &[space.clone()])
        .await
        .unwrap();
    let original = proof.monotonic_deadline();
    assert_eq!(
        proof.admission().unwrap().identity,
        CentralIdentity {
            issuer: issuer.clone(),
            subject: owner.to_string()
        }
    );
    assert!(proof.is_current());
    let again = native
        .owner_space_read(owner, &[space.clone()])
        .await
        .unwrap();
    assert_eq!(
        again.monotonic_deadline(),
        original,
        "policy cache preserves the original expiry"
    );
    assert_eq!(policy_reads.load(Ordering::SeqCst), 1);
    assert_eq!(identity_reads.load(Ordering::SeqCst), identity_baseline + 2);
    active.store(false, Ordering::SeqCst);
    assert!(
        native.owner_space_read(owner, &[space]).await.is_err(),
        "fresh disabled identity closes access despite a cached positive policy"
    );
    assert_eq!(policy_reads.load(Ordering::SeqCst), 1);
    assert_eq!(identity_reads.load(Ordering::SeqCst), identity_baseline + 3);
    assert!(
        native.is_current(),
        "owner read denial does not fabricate native token revocation"
    );
    let next = app::native_space_access::NativeRequest::verified(&verified, true, true).unwrap();
    assert!(
        matches!(
            next.require_known_owner(owner).await,
            Err(shared::AppError::Forbidden)
        ),
        "both policy flags off still require an active human owner on a new request"
    );
    server.abort();
    let _ = server.await;
}
