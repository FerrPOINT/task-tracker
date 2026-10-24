//! Transaction-local request context. Every SQL operation uses the same rule;
//! SQL strings are never parsed/classified as reads or writes by this adapter.
//! Permission/role/catalog decisions are owned by product database guards.
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr,
    ExecResult, IsolationLevel, QueryResult, Statement, TransactionError, TransactionTrait,
};
use std::{future::Future, pin::Pin, sync::Arc};
pub(crate) const EXPIRED: &str = "pdlc_request_authority_expired";
#[derive(Clone)]
struct BoundRequest {
    frame: Option<Arc<app::space_access::CentralRequestContext>>,
    native: Option<Arc<app::native_space_access::NativeRequest>>,
    document: Option<serde_json::Value>,
}
impl BoundRequest {
    fn capture() -> Result<Option<Self>, DbErr> {
        let frame =
            app::space_access::current_request().map_err(|_| DbErr::Custom(EXPIRED.into()))?;
        if let Some(frame) = frame {
            if !frame.is_mutation() {
                return Ok(None);
            }
            let document = if let Some(actor) = frame.spaces() {
                Some(
                    serde_json::json!({"schema_version":1,"kind":"human","user_id":frame.user().as_uuid(),
                    "identity":actor.identity(),"active_spaces":actor.active_spaces().map_err(|_|DbErr::Custom(EXPIRED.into()))?,
                    "readable_spaces":actor.readable_spaces().map_err(|_|DbErr::Custom(EXPIRED.into()))?,
                    "policy_revision":actor.policy_revision(),"writable":frame.allows_mutation(),"valid_until":frame.valid_until()}),
                )
            } else {
                None
            };
            return Ok(Some(Self {
                frame: Some(frame),
                native: None,
                document,
            }));
        }
        if let Some(native) =
            app::native_space_access::current().map_err(|_| DbErr::Custom(EXPIRED.into()))?
        {
            return Ok(Some(Self {
                frame: None,
                native: Some(native),
                document: None,
            }));
        }
        Ok(None)
    }
    fn current(&self) -> Result<(), DbErr> {
        let current = self.frame.as_ref().is_some_and(|f| f.is_current())
            || self.native.as_ref().is_some_and(|f| f.is_current());
        if current {
            Ok(())
        } else {
            Err(DbErr::Custom(EXPIRED.into()))
        }
    }
    async fn bind(&self, tx: &DatabaseTransaction) -> Result<(), DbErr> {
        self.current()?;
        let valid_until = match (&self.frame, &self.native) {
            (Some(frame), _) => frame.valid_until(),
            (_, Some(native)) => {
                native
                    .validity()
                    .map_err(|_| DbErr::Custom(EXPIRED.into()))?
                    .valid_until
            }
            _ => return Err(DbErr::Custom(EXPIRED.into())),
        };
        bind_commit_clock(tx, valid_until).await?;
        if tx
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE",
            ))
            .await?
            .is_none()
        {
            return Err(DbErr::Custom("space_admission_state_missing".into()));
        }
        if let Some(document) = self.document.as_ref() {
            tx.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('pdlc.tracker.write_actor',$1,true)",
                [document.to_string().into()],
            ))
            .await?;
        }
        if let Some(native) = self.native.as_ref() {
            let validity = native
                .validity()
                .map_err(|_| DbErr::Custom(EXPIRED.into()))?;
            let document = serde_json::json!({"schema_version":1,"identity":native.identity(),
                "scopes":native.scopes(),"writable":native.may_write(),"validity":validity});
            tx.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('pdlc.tracker.native_caller',$1,true)",
                [document.to_string().into()],
            ))
            .await?;
        }
        self.current()
    }
}
/// The DB checks after row waits, including time spent inside COMMIT itself.
pub(crate) async fn bind_commit_clock(
    tx: &DatabaseTransaction,
    valid_until: i64,
) -> Result<(), DbErr> {
    let until = chrono::DateTime::from_timestamp(valid_until, 0)
        .ok_or_else(|| DbErr::Custom(EXPIRED.into()))?;
    bind_commit_deadline(tx, until).await
}

pub(crate) async fn bind_commit_deadline(
    tx: &DatabaseTransaction,
    valid_until: chrono::DateTime<chrono::Utc>,
) -> Result<(), DbErr> {
    let fence = uuid::Uuid::new_v4();
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tracker_request_commit_fences(fence_id,valid_until) VALUES($1,$2)",
        [fence.into(), valid_until.into()],
    ))
    .await?;
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM tracker_request_commit_fences WHERE fence_id=$1",
        [fence.into()],
    ))
    .await?;
    Ok(())
}
pub struct GuardedDb {
    inner: DatabaseConnection,
}
impl GuardedDb {
    pub fn new(inner: DatabaseConnection) -> Self {
        Self { inner }
    }
    /// Explicit only: owner/reconcile/read-snapshot ports own their typed proof.
    /// A caller must not use this to bypass ordinary content authorization.
    pub(crate) fn raw(&self) -> &DatabaseConnection {
        &self.inner
    }
    async fn start(
        &self,
        scope: &Option<BoundRequest>,
        isolation: Option<IsolationLevel>,
        access: Option<AccessMode>,
    ) -> Result<DatabaseTransaction, DbErr> {
        let tx = self.inner.begin_with_config(isolation, access).await?;
        if let Some(scope) = scope {
            if let Err(error) = scope.bind(&tx).await {
                let _ = tx.rollback().await;
                return Err(error);
            }
        }
        Ok(tx)
    }
    async fn finish<T: Send>(
        &self,
        tx: DatabaseTransaction,
        scope: Option<BoundRequest>,
        result: Result<T, DbErr>,
    ) -> Result<T, DbErr> {
        let result = result.and_then(|value| {
            if let Some(scope) = scope {
                scope.current()?;
            }
            Ok(value)
        });
        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}
#[async_trait::async_trait]
impl ConnectionTrait for GuardedDb {
    fn get_database_backend(&self) -> DbBackend {
        self.inner.get_database_backend()
    }
    fn is_mock_connection(&self) -> bool {
        self.inner.is_mock_connection()
    }
    fn support_returning(&self) -> bool {
        self.inner.support_returning()
    }
    async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        let scope = BoundRequest::capture()?;
        if scope.is_none() {
            return self.inner.execute(stmt).await;
        }
        let tx = self.start(&scope, None, None).await?;
        let result = tx.execute(stmt).await;
        self.finish(tx, scope, result).await
    }
    async fn execute_unprepared(&self, query: &str) -> Result<ExecResult, DbErr> {
        let scope = BoundRequest::capture()?;
        if scope.is_none() {
            return self.inner.execute_unprepared(query).await;
        }
        let tx = self.start(&scope, None, None).await?;
        let result = tx.execute_unprepared(query).await;
        self.finish(tx, scope, result).await
    }
    async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        let scope = BoundRequest::capture()?;
        if scope.is_none() {
            return self.inner.query_one(stmt).await;
        }
        let tx = self.start(&scope, None, None).await?;
        let result = tx.query_one(stmt).await;
        self.finish(tx, scope, result).await
    }
    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let scope = BoundRequest::capture()?;
        if scope.is_none() {
            return self.inner.query_all(stmt).await;
        }
        let tx = self.start(&scope, None, None).await?;
        let result = tx.query_all(stmt).await;
        self.finish(tx, scope, result).await
    }
}
#[async_trait::async_trait]
impl TransactionTrait for GuardedDb {
    async fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
        let scope = BoundRequest::capture()?;
        self.start(&scope, None, None).await
    }
    async fn begin_with_config(
        &self,
        isolation: Option<IsolationLevel>,
        access: Option<AccessMode>,
    ) -> Result<DatabaseTransaction, DbErr> {
        let scope = BoundRequest::capture()?;
        self.start(&scope, isolation, access).await
    }
    async fn transaction<F, T, E>(&self, callback: F) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        self.transaction_with_config(callback, None, None).await
    }
    async fn transaction_with_config<F, T, E>(
        &self,
        callback: F,
        isolation: Option<IsolationLevel>,
        access: Option<AccessMode>,
    ) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        let scope = BoundRequest::capture().map_err(TransactionError::Connection)?;
        let tx = self
            .start(&scope, isolation, access)
            .await
            .map_err(TransactionError::Connection)?;
        match callback(&tx).await {
            Ok(value) => {
                if let Some(scope) = scope {
                    if let Err(error) = scope.current() {
                        let _ = tx.rollback().await;
                        return Err(TransactionError::Connection(error));
                    }
                }
                tx.commit().await.map_err(TransactionError::Connection)?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(TransactionError::Transaction(error))
            }
        }
    }
}

/// Creation is a dedicated typed port: no Project/links exist yet. The exact
/// requested set is known and must precede board/content creation in the same TX.
pub(crate) async fn bind_creation_context(
    tx: &DatabaseTransaction,
    project: shared::ProjectId,
    spaces: &[shared::space_access::SpaceRef],
    permit: &domain::project_spaces::SpaceWritePermit,
) -> Result<(), shared::AppError> {
    permit.require_set(spaces)?;
    bind_commit_clock(tx, permit.valid_until())
        .await
        .map_err(crate::db_error::map)?;
    if tx
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE",
        ))
        .await
        .map_err(shared::AppError::database)?
        .is_none()
    {
        return Err(shared::AppError::Unavailable(
            "space_admission_state_missing".into(),
        ));
    }
    let document = serde_json::json!({"schema_version":1,"kind":"create_project","project_id":project.as_uuid(),"user_id":permit.actor().as_uuid(),"identity":permit.identity(),"active_spaces":permit.active_spaces()?,"readable_spaces":permit.active_spaces()?,"requested_spaces":spaces,"policy_revision":permit.policy_revision(),"writable":true,"valid_until":permit.valid_until()});
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT set_config('pdlc.tracker.write_actor',$1,true)",
        [document.to_string().into()],
    ))
    .await
    .map_err(shared::AppError::database)?;
    permit.require_set(spaces)?;
    Ok(())
}

#[cfg(test)]
mod request_clock_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires disposable TT_CLOCK_TEST_DATABASE_URL"]
    async fn original_request_clock_is_checked_inside_commit() {
        let url =
            std::env::var("TT_CLOCK_TEST_DATABASE_URL").expect("owned clock QA database required");
        let db = sea_orm::Database::connect(url).await.unwrap();
        let name = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT current_database() AS name",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            name.try_get::<String>("", "name").unwrap(),
            "tracker_clock_test",
            "refuse working database"
        );
        db.execute_unprepared(include_str!("../../migration/src/request_commit_clock.sql"))
            .await
            .unwrap();
        db.execute_unprepared("CREATE TABLE qa_request_clock_effects(id integer PRIMARY KEY); CREATE SEQUENCE qa_request_clock_seen; CREATE FUNCTION qa_request_clock_delay() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('qa_request_clock_seen'); PERFORM pg_sleep(2.1); RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER a_qa_request_clock_delay AFTER INSERT ON tracker_request_commit_fences DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION qa_request_clock_delay();").await.unwrap();
        let tx = db.begin().await.unwrap();
        let until = chrono::Utc::now().timestamp() + 2;
        bind_commit_clock(&tx, until).await.unwrap();
        tx.execute_unprepared("INSERT INTO qa_request_clock_effects VALUES(1)")
            .await
            .unwrap();
        assert!(
            chrono::Utc::now().timestamp() < until,
            "the client check must still be current before COMMIT"
        );
        let error = tx
            .commit()
            .await
            .expect_err("expiry inside COMMIT must reject the effect");
        assert!(matches!(
            crate::db_error::map(error),
            shared::AppError::Forbidden
        ));
        let row=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS seen FROM qa_request_clock_seen")).await.unwrap().unwrap();
        assert_eq!(
            row.try_get::<i64>("", "seen").unwrap(),
            1,
            "the real adapter must enter the deferred COMMIT event"
        );
        db.execute_unprepared("DROP TRIGGER a_qa_request_clock_delay ON tracker_request_commit_fences; DROP FUNCTION qa_request_clock_delay(); DROP SEQUENCE qa_request_clock_seen;").await.unwrap();
        let tx = db.begin().await.unwrap();
        bind_commit_clock(&tx, chrono::Utc::now().timestamp() + 30)
            .await
            .unwrap();
        tx.execute_unprepared("INSERT INTO qa_request_clock_effects VALUES(2)")
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let row=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM tracker_request_commit_fences) AS fences, (SELECT array_agg(id ORDER BY id) FROM qa_request_clock_effects) AS effects")).await.unwrap().unwrap();
        assert_eq!(
            row.try_get::<i64>("", "fences").unwrap(),
            0,
            "no durable fence rows"
        );
        assert_eq!(
            row.try_get::<Vec<i32>>("", "effects").unwrap(),
            vec![2],
            "only the fresh operation is durable"
        );
        db.close().await.unwrap();
    }
}
