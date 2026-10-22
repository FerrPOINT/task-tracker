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
