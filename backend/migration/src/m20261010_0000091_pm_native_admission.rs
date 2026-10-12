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
    IF NEW.state IS DISTINCT FROM OLD.state AND NOT EXISTS(
      SELECT 1 FROM sdlc_pm_native_admissions a WHERE a.execution_id=OLD.pm_execution_id
        AND a.assignment_id::text=OLD.state #>> '{assignment,assignment_id}'
        AND a.assignment_version::text=OLD.state #>> '{assignment,version}'
        AND a.proof #>> '{identity,task_ref}'=OLD.task_id::text) THEN
      RAISE EXCEPTION 'SDLC reserved PM requires verified admission' USING ERRCODE='23514';
    END IF;
  END IF;
  RETURN NEW;
END $$;";

const RESERVED_GATE: &str = "CREATE OR REPLACE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
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
        manager.get_connection().execute_unprepared("CREATE TABLE sdlc_pm_native_admissions (
          execution_id uuid NOT NULL REFERENCES sdlc_pm_executions(execution_id) ON DELETE RESTRICT,
          session_run_id uuid NOT NULL, observation_ref uuid NOT NULL UNIQUE,
          assignment_id uuid NOT NULL, assignment_version bigint NOT NULL CHECK(assignment_version>0),
          fence bigint NOT NULL CHECK(fence>0), proof jsonb NOT NULL CHECK(jsonb_typeof(proof)='object'),
          created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
          PRIMARY KEY(execution_id,session_run_id),
          CHECK ((proof->>'contract_version'='1' AND proof->>'session_run_id'=session_run_id::text
            AND proof->>'observation_ref'=observation_ref::text AND proof->>'fence'=fence::text
            AND proof #>> '{identity,execution_ref}'=execution_id::text
            AND proof #>> '{identity,assignment_ref}'=assignment_id::text
            AND proof #>> '{identity,assignment_revision}'=assignment_version::text
            AND proof->>'configuration_sha256' ~ '^[0-9a-f]{64}$') IS TRUE));
          CREATE FUNCTION sdlc_native_admission_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
          BEGIN RAISE EXCEPTION 'SDLC native admission history is immutable' USING ERRCODE='23514'; END $$;
          CREATE TRIGGER sdlc_native_admission_immutable BEFORE UPDATE OR DELETE ON sdlc_pm_native_admissions
            FOR EACH ROW EXECUTE FUNCTION sdlc_native_admission_immutable();").await?;
        manager
            .get_connection()
            .execute_unprepared(VERIFIED_GATE)
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DO $$ BEGIN
          IF EXISTS(SELECT 1 FROM sdlc_pm_native_admissions) THEN
            RAISE EXCEPTION 'SDLC native admission history requires reconciliation before downgrade' USING ERRCODE='23514';
          END IF; END $$;").await?;
        manager
            .get_connection()
            .execute_unprepared(RESERVED_GATE)
            .await?;
        manager.get_connection().execute_unprepared(
            "DROP TABLE sdlc_pm_native_admissions; DROP FUNCTION sdlc_native_admission_immutable();").await?;
        Ok(())
    }
}

#[cfg(test)]
mod owned_schema_test {
    use super::*;
    #[tokio::test]
    #[ignore = "requires explicit owned native Tracker QA database"]
    async fn upgrade_owned_native_tracker_is_idempotent() {
        let db =
            sea_orm::Database::connect(std::env::var("TRACKER_NATIVE_ADMISSION_TEST_URL").unwrap())
                .await
                .unwrap();
        crate::Migrator::up(&db, None).await.unwrap();
        crate::Migrator::up(&db, None).await.unwrap();
        let row=db.query_one(sea_orm::Statement::from_string(sea_orm::DatabaseBackend::Postgres,
            "SELECT pg_get_functiondef('sdlc_pm_reservation_gate()'::regprocedure) AS definition"))
            .await.unwrap().unwrap();
        let definition: String = row.try_get("", "definition").unwrap();
        assert!(definition.contains("sdlc_pm_native_admissions"));
        assert!(definition.contains("ownership is immutable"));
    }

    #[tokio::test]
    #[ignore = "requires explicit owned native Tracker QA database with accepted history"]
    async fn admitted_history_and_task_ownership_are_immutable() {
        use sea_orm::TransactionTrait;
        let db =
            sea_orm::Database::connect(std::env::var("TRACKER_NATIVE_ADMISSION_TEST_URL").unwrap())
                .await
                .unwrap();
        let row = db
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT count(*) AS total FROM sdlc_pm_native_admissions",
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(row.try_get::<i64>("", "total").unwrap() > 0);
        for sql in [
            "UPDATE sdlc_pm_native_admissions SET fence=fence+1",
            "DELETE FROM sdlc_pm_native_admissions",
            "UPDATE sdlc_tasks SET state=jsonb_set(state,'{assignment,agent_id}',to_jsonb(gen_random_uuid()::text)) WHERE pm_execution_id IN (SELECT execution_id FROM sdlc_pm_native_admissions)",
            "UPDATE sdlc_tasks SET state=jsonb_set(state,'{waiting_reason}','\"altered\"') WHERE pm_execution_id IS NOT NULL AND pm_execution_id NOT IN (SELECT execution_id FROM sdlc_pm_native_admissions)",
        ] {
            let tx = db.begin().await.unwrap();
            let error = tx.execute_unprepared(sql).await.unwrap_err();
            assert!(error.to_string().contains("SDLC"), "{error}");
            tx.rollback().await.unwrap();
        }
        let tx = db.begin().await.unwrap();
        let manager = SchemaManager::new(&tx);
        let error = Migration.down(&manager).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires reconciliation before downgrade")
        );
        tx.rollback().await.unwrap();
    }
}
