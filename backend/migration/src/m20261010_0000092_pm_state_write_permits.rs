use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const VERIFIED_GATE: &str =
    "CREATE OR REPLACE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF OLD.pm_execution_id IS NOT NULL THEN
    IF NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
      OR NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
      OR NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state
      OR NEW.state->'assignment' IS DISTINCT FROM OLD.state->'assignment'
      OR NEW.state->'task_id' IS DISTINCT FROM OLD.state->'task_id'
      OR NEW.state->'root_task_id' IS DISTINCT FROM OLD.state->'root_task_id'
      OR NEW.state->'project_id' IS DISTINCT FROM OLD.state->'project_id'
      OR NEW.state->'tracker_instance_id' IS DISTINCT FROM OLD.state->'tracker_instance_id'
      OR NEW.state->'owner_subject' IS DISTINCT FROM OLD.state->'owner_subject' THEN
      RAISE EXCEPTION 'SDLC reserved PM ownership is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.state IS DISTINCT FROM OLD.state THEN
      IF NOT EXISTS(
        SELECT 1 FROM sdlc_pm_native_admissions a WHERE a.execution_id=OLD.pm_execution_id
          AND a.assignment_id::text=OLD.state #>> '{assignment,assignment_id}'
          AND a.assignment_version::text=OLD.state #>> '{assignment,version}'
          AND a.proof #>> '{identity,task_ref}'=OLD.task_id::text) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires verified admission' USING ERRCODE='23514';
      END IF;
      IF NOT EXISTS(
        SELECT 1 FROM sdlc_pm_state_write_permits p WHERE p.task_id=OLD.task_id
          AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires an authorized state write' USING ERRCODE='23514';
      END IF;
      DELETE FROM sdlc_pm_state_write_permits WHERE task_id=OLD.task_id
        AND transaction_id=pg_current_xact_id() AND next_state=NEW.state;
    END IF;
  END IF;
  RETURN NEW;
END $$;";

const RESERVED_GATE: &str =
    "CREATE OR REPLACE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF OLD.pm_execution_id IS NOT NULL AND
    (NEW.state IS DISTINCT FROM OLD.state OR NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
      OR NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
      OR NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state) THEN
    RAISE EXCEPTION 'SDLC reserved PM requires verified admission' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
END $$;";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE sdlc_pm_state_write_permits (
                  task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
                  transaction_id xid8 NOT NULL,
                  next_state jsonb NOT NULL,
                  PRIMARY KEY(task_id,transaction_id))",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(VERIFIED_GATE)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DO $$ BEGIN
                  IF EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits) THEN
                    RAISE EXCEPTION 'pending PM state-write permits require reconciliation before downgrade'
                      USING ERRCODE='23514';
                  END IF;
                  IF EXISTS(SELECT 1 FROM sdlc_pm_native_admissions) THEN
                    RAISE EXCEPTION 'PM admission history requires reconciliation before state-write guard downgrade'
                      USING ERRCODE='23514';
                  END IF;
                END $$;",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(RESERVED_GATE)
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE sdlc_pm_state_write_permits")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod owned_state_write_test {
    use super::*;
    use sea_orm::TransactionTrait;

    #[tokio::test]
    #[ignore = "requires a disposable database named tracker127_state_write_test"]
    async fn exact_transaction_permit_is_required_and_consumed() {
        let db = sea_orm::Database::connect(std::env::var("TRACKER_STATE_WRITE_TEST_URL").unwrap())
            .await
            .unwrap();
        let database_name = db
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT current_database() AS name",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<String>("", "name")
            .unwrap();
        assert_eq!(database_name, "tracker127_state_write_test");
        db.execute_unprepared("CREATE SCHEMA tracker127_state_write_test")
            .await
            .unwrap();

        let task_id = "00000000-0000-4000-8000-000000000101";
        let execution_id = "00000000-0000-4000-8000-000000000102";
        let assignment_id = "00000000-0000-4000-8000-000000000103";
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("SET LOCAL search_path TO tracker127_state_write_test")
            .await
            .unwrap();
        tx.execute_unprepared(
            "CREATE TABLE sdlc_pm_executions(execution_id uuid PRIMARY KEY);
            CREATE TABLE sdlc_tasks(task_id uuid PRIMARY KEY, pm_owner_version integer NOT NULL,
              pm_execution_id uuid, pm_admission_state text, state jsonb NOT NULL);
            CREATE TABLE sdlc_pm_native_admissions(execution_id uuid, assignment_id uuid,
              assignment_version bigint, proof jsonb);
            CREATE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
              BEGIN RETURN NEW; END $$;
            CREATE TRIGGER sdlc_pm_reservation_gate BEFORE UPDATE ON sdlc_tasks
              FOR EACH ROW EXECUTE FUNCTION sdlc_pm_reservation_gate();",
        )
        .await
        .unwrap();
        tx.execute_unprepared(&format!("INSERT INTO sdlc_pm_executions VALUES('{execution_id}');
            INSERT INTO sdlc_tasks VALUES('{task_id}',1,'{execution_id}','reserved',
              '{{\"assignment\":{{\"assignment_id\":\"{assignment_id}\",\"version\":1}},\"task_id\":\"{task_id}\"}}');
            INSERT INTO sdlc_pm_native_admissions VALUES('{execution_id}','{assignment_id}',1,
              '{{\"identity\":{{\"task_ref\":\"{task_id}\"}}}}');"))
            .await
            .unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("SET LOCAL search_path TO tracker127_state_write_test")
            .await
            .unwrap();
        let unscoped = tx
            .execute_unprepared(&format!("UPDATE sdlc_tasks SET state=jsonb_set(state,'{{waiting_reason}}','\"unscoped\"') WHERE task_id='{task_id}'"))
            .await
            .unwrap_err();
        assert!(unscoped.to_string().contains("authorized state write"));
        tx.rollback().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("SET LOCAL search_path TO tracker127_state_write_test")
            .await
            .unwrap();
        // A mismatched next-state permit cannot authorize a different update.
        tx.execute_unprepared(&format!("INSERT INTO sdlc_pm_state_write_permits
            SELECT '{task_id}',pg_current_xact_id(),jsonb_set(state,'{{waiting_reason}}','\"permitted\"')
            FROM sdlc_tasks WHERE task_id='{task_id}'"))
            .await
            .unwrap();
        let mismatch = tx
            .execute_unprepared(&format!("UPDATE sdlc_tasks SET state=jsonb_set(state,'{{waiting_reason}}','\"different\"') WHERE task_id='{task_id}'"))
            .await
            .unwrap_err();
        assert!(mismatch.to_string().contains("authorized state write"));
        tx.rollback().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("SET LOCAL search_path TO tracker127_state_write_test")
            .await
            .unwrap();
        tx.execute_unprepared(&format!("INSERT INTO sdlc_pm_state_write_permits
            SELECT '{task_id}',pg_current_xact_id(),jsonb_set(state,'{{waiting_reason}}','\"permitted\"')
            FROM sdlc_tasks WHERE task_id='{task_id}'"))
            .await
            .unwrap();
        tx.execute_unprepared(&format!("UPDATE sdlc_tasks SET state=jsonb_set(state,'{{waiting_reason}}','\"permitted\"') WHERE task_id='{task_id}'"))
            .await
            .unwrap();
        let permits = tx
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT count(*) AS total FROM sdlc_pm_state_write_permits",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "total")
            .unwrap();
        assert_eq!(permits, 0, "trigger consumes the one-use permit");
        let reused = tx
            .execute_unprepared(&format!("UPDATE sdlc_tasks SET state=jsonb_set(state,'{{waiting_reason}}','\"reused\"') WHERE task_id='{task_id}'"))
            .await
            .unwrap_err();
        assert!(reused.to_string().contains("authorized state write"));
        tx.rollback().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("SET LOCAL search_path TO tracker127_state_write_test")
            .await
            .unwrap();
        let downgrade = Migration.down(&SchemaManager::new(&tx)).await.unwrap_err();
        assert!(
            downgrade
                .to_string()
                .contains("reconciliation before state-write guard downgrade")
        );
        tx.rollback().await.unwrap();

        db.execute_unprepared("DROP SCHEMA tracker127_state_write_test CASCADE")
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires an explicitly owned native Tracker QA database"]
    async fn admission_history_does_not_authorize_unscoped_task_updates() {
        let db =
            sea_orm::Database::connect(std::env::var("TRACKER_NATIVE_ADMISSION_TEST_URL").unwrap())
                .await
                .unwrap();
        crate::Migrator::up(&db, None).await.unwrap();
        let count = db
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT count(*) AS total FROM sdlc_pm_native_admissions",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "total")
            .unwrap();
        assert!(count > 0, "owned QA database must contain accepted history");

        let tx = db.begin().await.unwrap();
        let error = tx
            .execute_unprepared(
                "UPDATE sdlc_tasks SET state=jsonb_set(state,'{waiting_reason}','\"unscoped\"')
                 WHERE pm_execution_id IN (SELECT execution_id FROM sdlc_pm_native_admissions)",
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("authorized state write"),
            "{error}"
        );
        tx.rollback().await.unwrap();

        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(
            "INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
             SELECT task_id,pg_current_xact_id(),jsonb_set(state,'{waiting_reason}','\"authorized\"')
             FROM sdlc_tasks WHERE pm_execution_id IN (SELECT execution_id FROM sdlc_pm_native_admissions)
             ORDER BY task_id LIMIT 1",
        )
        .await
        .unwrap();
        tx.execute_unprepared(
            "UPDATE sdlc_tasks SET state=jsonb_set(state,'{waiting_reason}','\"authorized\"')
             WHERE task_id=(SELECT task_id FROM sdlc_tasks WHERE pm_execution_id IN
               (SELECT execution_id FROM sdlc_pm_native_admissions) ORDER BY task_id LIMIT 1)",
        )
        .await
        .unwrap();
        let remaining = tx
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT count(*) AS total FROM sdlc_pm_state_write_permits",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "total")
            .unwrap();
        assert_eq!(
            remaining, 0,
            "the permit must be consumed with the state update"
        );
        tx.rollback().await.unwrap();

        let tx = db.begin().await.unwrap();
        let error = Migration.down(&SchemaManager::new(&tx)).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("reconciliation before state-write guard downgrade")
        );
        tx.rollback().await.unwrap();
    }
}
