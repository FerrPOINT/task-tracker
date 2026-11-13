//! Manual wire-fault regression: COMMIT reaches PostgreSQL, its acknowledgement does not.
//! The proxy is confined to this test process and a disposable, guarded database.
use domain::{Attachment, AttachmentSaveError};
use infra::repos::SeaOrmRepositories;
use sea_orm::{Database, sqlx};
use sqlx::postgres::PgPoolOptions;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UnixStream},
};
use uuid::Uuid;

struct FaultProxy {
    port: u16,
    lost: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for FaultProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl FaultProxy {
    async fn start(socket: String, refuse_readback: bool, drop_commit_failure: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let lost = Arc::new(AtomicBool::new(false));
        let state = lost.clone();
        let task = tokio::spawn(async move {
            loop {
                let (client, _) = listener.accept().await.unwrap();
                let lost = state.clone();
                let socket = socket.clone();
                tokio::spawn(async move {
                    if refuse_readback && lost.load(Ordering::SeqCst) {
                        return;
                    }
                    let server = UnixStream::connect(socket).await.unwrap();
                    let (mut cr, mut cw) = client.into_split();
                    let (mut sr, mut sw) = server.into_split();
                    let forward = async {
                        let _ = tokio::io::copy(&mut cr, &mut sw).await;
                    };
                    let reply = async {
                        loop {
                            let mut tag = [0];
                            if sr.read_exact(&mut tag).await.is_err() {
                                break;
                            }
                            let mut length = [0; 4];
                            if sr.read_exact(&mut length).await.is_err() {
                                break;
                            }
                            let length_value = u32::from_be_bytes(length) as usize;
                            assert!((4..=16 * 1024 * 1024).contains(&length_value));
                            let mut body = vec![0; length_value - 4];
                            if sr.read_exact(&mut body).await.is_err() {
                                break;
                            }
                            let commit_reply = tag[0] == b'C' && body == b"COMMIT\0";
                            let deferred_fk_failure = drop_commit_failure
                                && tag[0] == b'E'
                                && body.windows(7).any(|field| field == b"C23503\0");
                            if (commit_reply || deferred_fk_failure)
                                && !lost.swap(true, Ordering::SeqCst)
                            {
                                break;
                            }
                            if cw.write_all(&tag).await.is_err()
                                || cw.write_all(&length).await.is_err()
                                || cw.write_all(&body).await.is_err()
                            {
                                break;
                            }
                        }
                    };
                    tokio::select! {_=forward=>{},_=reply=>{}}
                });
            }
        });
        Self { port, lost, task }
    }
}
fn value(issue: Uuid, author: Uuid) -> Attachment {
    let id = shared::AttachmentId::new();
    Attachment {
        id,
        issue_id: shared::IssueId::from_uuid(issue),
        author_id: shared::UserId::from_uuid(author),
        file_name: "Кавычки ' и пробел.txt".into(),
        content_type: "text/plain".into(),
        size_bytes: 7,
        storage_key: format!("{id}-fixture").into(),
        created_at: shared::now(),
    }
}
#[tokio::test]
#[ignore = "manual owned tracker_attachment_test PostgreSQL wire-fault regression"]
async fn metadata_commit_acknowledgement_is_resolved_before_blob_cleanup_is_allowed() {
    let url = std::env::var("TRACKER_ATTACHMENT_TEST_DATABASE_URL").expect("owned fixture URL");
    let control = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT current_database()")
            .fetch_one(&control)
            .await
            .unwrap(),
        "tracker_attachment_test"
    );
    let schema = format!("attachments_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!("CREATE SCHEMA {schema};CREATE TABLE {schema}.issues(id uuid PRIMARY KEY,deleted_at timestamptz);CREATE TABLE {schema}.users(id uuid PRIMARY KEY);CREATE TABLE {schema}.attachments(id uuid PRIMARY KEY,issue_id uuid NOT NULL REFERENCES {schema}.issues(id),author_id uuid NOT NULL REFERENCES {schema}.users(id) DEFERRABLE INITIALLY DEFERRED,file_name text NOT NULL,content_type text NOT NULL,size_bytes bigint NOT NULL CHECK(size_bytes>=0),storage_key text NOT NULL,created_at timestamptz NOT NULL)")).execute(&control).await.unwrap();
    let issue = Uuid::new_v4();
    let author = Uuid::new_v4();
    sqlx::query(&format!("INSERT INTO {schema}.issues(id) VALUES($1)"))
        .bind(issue)
        .execute(&control)
        .await
        .unwrap();
    sqlx::query(&format!("INSERT INTO {schema}.users(id) VALUES($1)"))
        .bind(author)
        .execute(&control)
        .await
        .unwrap();
    let socket =
        std::env::var("TRACKER_TEST_POSTGRES_SOCKET").expect("owned socket") + "/.s.PGSQL.5432";
    for refuse_readback in [false, true] {
        let proxy = FaultProxy::start(socket.clone(), refuse_readback, false).await;
        let proxied = format!(
            "postgres://qa@127.0.0.1:{}/tracker_attachment_test?sslmode=disable&options=-csearch_path%3D{schema}",
            proxy.port
        );
        let mut options = sea_orm::ConnectOptions::new(proxied);
        options
            .max_connections(1)
            .min_connections(0)
            .connect_timeout(std::time::Duration::from_millis(500))
            .acquire_timeout(std::time::Duration::from_millis(500));
        let db = Database::connect(options).await.unwrap();
        let connection_pool = db.get_postgres_connection_pool().clone();
        let repositories = SeaOrmRepositories::new(db);
        let item = value(issue, author);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            repositories.attachments.save(&item),
        )
        .await
        .unwrap();
        assert!(
            proxy.lost.load(Ordering::SeqCst),
            "the real COMMIT response must have been dropped"
        );
        if refuse_readback {
            assert!(
                matches!(result, Err(AttachmentSaveError::Unknown(_))),
                "unavailable readback must preserve uncertainty: {result:?}"
            );
        } else {
            assert_eq!(
                result.unwrap(),
                item.id,
                "readback resolves the original committed ID"
            );
        }
        let saved: Uuid =
            sqlx::query_scalar(&format!("SELECT id FROM {schema}.attachments WHERE id=$1"))
                .bind(item.id.as_uuid())
                .fetch_one(&control)
                .await
                .unwrap();
        assert_eq!(saved, item.id.as_uuid());
        drop(repositories);
        connection_pool.close().await;
        drop(proxy);
    }
    // INSERT succeeds, then a deferred FK aborts COMMIT. Drop that error reply
    // too; only a fresh, locked original-ID readback permits blob cleanup.
    let proxy = FaultProxy::start(socket.clone(), false, true).await;
    let proxied = format!(
        "postgres://qa@127.0.0.1:{}/tracker_attachment_test?sslmode=disable&options=-csearch_path%3D{schema}",
        proxy.port
    );
    let mut options = sea_orm::ConnectOptions::new(proxied);
    options
        .max_connections(1)
        .min_connections(0)
        .connect_timeout(std::time::Duration::from_millis(500))
        .acquire_timeout(std::time::Duration::from_millis(500));
    let db = Database::connect(options).await.unwrap();
    let connection_pool = db.get_postgres_connection_pool().clone();
    let repositories = SeaOrmRepositories::new(db);
    let rejected = value(issue, Uuid::new_v4());
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        repositories.attachments.save(&rejected),
    )
    .await
    .unwrap();
    assert!(
        proxy.lost.load(Ordering::SeqCst),
        "the deferred COMMIT failure response must be lost"
    );
    assert!(
        matches!(outcome, Err(AttachmentSaveError::NotCommitted(_))),
        "confirmed absent original ID must resolve rollback: {outcome:?}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(&format!(
            "SELECT count(*) FROM {schema}.attachments WHERE id=$1"
        ))
        .bind(rejected.id.as_uuid())
        .fetch_one(&control)
        .await
        .unwrap(),
        0
    );
    drop(repositories);
    connection_pool.close().await;
    drop(proxy);
    let mut direct = reqwest::Url::parse(&url).unwrap();
    direct
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let direct = direct.to_string();
    let db = Database::connect(direct).await.unwrap();
    let connection_pool = db.get_postgres_connection_pool().clone();
    let repositories = SeaOrmRepositories::new(db);
    let mut rejected = value(issue, author);
    rejected.size_bytes = -1;
    assert!(matches!(
        repositories.attachments.save(&rejected).await,
        Err(AttachmentSaveError::NotCommitted(_))
    ));
    let count: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM {schema}.attachments WHERE id=$1"
    ))
    .bind(rejected.id.as_uuid())
    .fetch_one(&control)
    .await
    .unwrap();
    assert_eq!(count, 0);
    // The same parent lock serializes saves, while a different task remains usable.
    let other = Uuid::new_v4();
    sqlx::query(&format!("INSERT INTO {schema}.issues(id) VALUES($1)"))
        .bind(other)
        .execute(&control)
        .await
        .unwrap();
    let mut barrier = control.begin().await.unwrap();
    sqlx::query(&format!(
        "SELECT id FROM {schema}.issues WHERE id=$1 FOR UPDATE"
    ))
    .bind(issue)
    .execute(&mut *barrier)
    .await
    .unwrap();
    let waiting = value(issue, author);
    let writer = repositories.attachments.clone();
    let pending = tokio::spawn(async move { writer.save(&waiting).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!pending.is_finished());
    let independent = value(other, author);
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        repositories.attachments.save(&independent),
    )
    .await
    .unwrap()
    .unwrap();
    barrier.commit().await.unwrap();
    pending.await.unwrap().unwrap();
    drop(repositories);
    connection_pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
