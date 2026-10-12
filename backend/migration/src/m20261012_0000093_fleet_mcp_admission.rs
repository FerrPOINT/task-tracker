use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UPGRADE: &str = r#"ALTER TABLE sdlc_pm_native_admissions
    DROP CONSTRAINT sdlc_pm_native_admissions_check,
    ADD CONSTRAINT sdlc_pm_native_admissions_check CHECK (((proof->>'contract_version'='1' AND proof->>'session_run_id'=session_run_id::text
              AND proof->>'observation_ref'=observation_ref::text AND proof->>'fence'=fence::text
              AND proof #>> '{identity,execution_ref}'=execution_id::text
              AND proof #>> '{identity,assignment_ref}'=assignment_id::text
              AND proof #>> '{identity,assignment_revision}'=assignment_version::text
              AND proof->>'configuration_sha256' ~ '^[0-9a-f]{64}$')
            OR (proof->>'contract_version'='2' AND proof->>'profile'='fleet_mcp_custody_v1'
              AND proof->>'session_run_id'=session_run_id::text
              AND proof->>'observation_ref'=observation_ref::text AND proof->>'fence'=fence::text
              AND proof #>> '{identity,execution_ref}'=execution_id::text
              AND proof #>> '{identity,assignment_ref}'=assignment_id::text
              AND proof #>> '{identity,assignment_revision}'=assignment_version::text
              AND proof->>'configuration_sha256' ~ '^[0-9a-f]{64}$'
              AND proof->>'dispatch_request_sha256' ~ '^[0-9a-f]{64}$'
              AND proof #>> '{profile_configuration,evidence_basis}'='fleet_effective_config_and_accepted_run'
              AND proof #>> '{profile_configuration,runtime_source_revision}'='bbaf7af5c83546d19f8060f4097d3bb25cd1a3c3'
              AND proof #>> '{profile_configuration,mcp_server}'='fleet_pm'
              AND proof #> '{profile_configuration,platform_toolsets}'='["fleet_pm"]'::jsonb
              AND proof #> '{profile_configuration,advertised_server_tools}'='["tracker_clarifications","tracker_context","tracker_publish_question","tracker_publish_revision","tracker_requirements","workflow_step"]'::jsonb
              AND proof #> '{profile_configuration,configured_wire_tools}'='["mcp__fleet_pm__tracker_clarifications","mcp__fleet_pm__tracker_context","mcp__fleet_pm__tracker_publish_question","mcp__fleet_pm__tracker_publish_revision","mcp__fleet_pm__tracker_requirements","mcp__fleet_pm__workflow_step"]'::jsonb)) IS TRUE);"#;

const DOWNGRADE: &str = r#"ALTER TABLE sdlc_pm_native_admissions
    DROP CONSTRAINT sdlc_pm_native_admissions_check,
    ADD CONSTRAINT sdlc_pm_native_admissions_check CHECK ((proof->>'contract_version'='1' AND proof->>'session_run_id'=session_run_id::text
            AND proof->>'observation_ref'=observation_ref::text AND proof->>'fence'=fence::text
            AND proof #>> '{identity,execution_ref}'=execution_id::text
            AND proof #>> '{identity,assignment_ref}'=assignment_id::text
            AND proof #>> '{identity,assignment_revision}'=assignment_version::text
            AND proof->>'configuration_sha256' ~ '^[0-9a-f]{64}$') IS TRUE);"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UPGRADE).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("DO $$ BEGIN
          IF EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE proof->>'contract_version'='2') THEN
            RAISE EXCEPTION 'Fleet MCP admission history requires reconciliation before downgrade' USING ERRCODE='23514';
          END IF; END $$;").await?;
        manager
            .get_connection()
            .execute_unprepared(DOWNGRADE)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, DatabaseBackend, TransactionTrait};
    use serde_json::{Value, json};

    async fn insert(db: &impl ConnectionTrait, proof: &Value) -> Result<(), DbErr> {
        db.execute(sea_orm::Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO sdlc_pm_native_admissions(execution_id,session_run_id,observation_ref,
              assignment_id,assignment_version,fence,proof)
             SELECT (p #>> '{identity,execution_ref}')::uuid,(p->>'session_run_id')::uuid,
              (p->>'observation_ref')::uuid,(p #>> '{identity,assignment_ref}')::uuid,
              (p #>> '{identity,assignment_revision}')::bigint,(p->>'fence')::bigint,p
             FROM (SELECT $1::jsonb AS p) input",
            [proof.clone().into()],
        ))
        .await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires explicit empty owned Tracker MCP upgrade QA database"]
    async fn existing_v1_schema_upgrades_without_rewriting_history() {
        let url = std::env::var("TRACKER_MCP_ADMISSION_TEST_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("tracker_mcp_qa_")
        );
        let db = sea_orm::Database::connect(url).await.unwrap();
        let tx = db.begin().await.unwrap();
        // Only the prerequisite identity table is synthetic. Both admission
        // migrations, their constraints and the immutable trigger are real.
        tx.execute_unprepared(
            "CREATE TABLE sdlc_pm_executions(execution_id uuid PRIMARY KEY);
          INSERT INTO sdlc_pm_executions VALUES ('10000000-0000-4000-8000-000000000001')",
        )
        .await
        .unwrap();
        let manager = SchemaManager::new(&tx);
        crate::m20261010_0000091_pm_native_admission::Migration
            .up(&manager)
            .await
            .unwrap();
        let v1 = json!({"contract_version":1,"session_run_id":"30000000-0000-4000-8000-000000000001",
            "observation_ref":"40000000-0000-4000-8000-000000000001","fence":1,
            "identity":{"execution_ref":"10000000-0000-4000-8000-000000000001",
                "assignment_ref":"20000000-0000-4000-8000-000000000001","assignment_revision":1},
            "configuration_sha256":"b".repeat(64)});
        insert(&tx, &v1).await.unwrap();
        let mut v2 = v1.clone();
        v2["contract_version"] = json!(2);
        v2["profile"] = json!("fleet_mcp_custody_v1");
        v2["session_run_id"] = json!("30000000-0000-4000-8000-000000000002");
        v2["observation_ref"] = json!("40000000-0000-4000-8000-000000000002");
        v2["fence"] = json!(2);
        v2["dispatch_request_sha256"] = json!("c".repeat(64));
        v2["profile_configuration"] = json!({
            "evidence_basis":"fleet_effective_config_and_accepted_run",
            "runtime_source_revision":"bbaf7af5c83546d19f8060f4097d3bb25cd1a3c3",
            "mcp_server":"fleet_pm","platform_toolsets":["fleet_pm"],
            "advertised_server_tools":["tracker_clarifications","tracker_context","tracker_publish_question",
                "tracker_publish_revision","tracker_requirements","workflow_step"],
            "configured_wire_tools":["mcp__fleet_pm__tracker_clarifications","mcp__fleet_pm__tracker_context",
                "mcp__fleet_pm__tracker_publish_question","mcp__fleet_pm__tracker_publish_revision",
                "mcp__fleet_pm__tracker_requirements","mcp__fleet_pm__workflow_step"]});
        tx.execute_unprepared("SAVEPOINT before_v2").await.unwrap();
        assert!(
            insert(&tx, &v2)
                .await
                .unwrap_err()
                .to_string()
                .contains("sdlc_pm_native_admissions_check")
        );
        tx.execute_unprepared("ROLLBACK TO SAVEPOINT before_v2")
            .await
            .unwrap();
        Migration.up(&manager).await.unwrap();
        insert(&tx, &v2).await.unwrap();
        let saved = tx
            .query_one(sea_orm::Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT proof FROM sdlc_pm_native_admissions WHERE fence=1",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.try_get::<Value>("", "proof").unwrap(), v1);
        tx.execute_unprepared("SAVEPOINT before_down")
            .await
            .unwrap();
        assert!(
            Migration
                .down(&manager)
                .await
                .unwrap_err()
                .to_string()
                .contains("requires reconciliation")
        );
        tx.execute_unprepared("ROLLBACK TO SAVEPOINT before_down")
            .await
            .unwrap();
        tx.execute_unprepared("SAVEPOINT before_delete")
            .await
            .unwrap();
        assert!(
            tx.execute_unprepared("DELETE FROM sdlc_pm_native_admissions")
                .await
                .unwrap_err()
                .to_string()
                .contains("history is immutable")
        );
        tx.execute_unprepared("ROLLBACK TO SAVEPOINT before_delete")
            .await
            .unwrap();
        let saved = tx
            .query_one(sea_orm::Statement::from_string(
                DatabaseBackend::Postgres,
                "SELECT count(*) AS total FROM sdlc_pm_native_admissions",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.try_get::<i64>("", "total").unwrap(), 2);
        tx.rollback().await.unwrap();
    }
}
