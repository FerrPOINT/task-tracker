use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("configured_cli.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "configured CLI history is forward-only".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn protocol_is_creation_only_and_never_creates_a_native_lease() {
        let sql = include_str!("configured_cli.sql");
        assert!(sql.contains(
            "OLD.state->'interaction_protocol' IS DISTINCT FROM NEW.state->'interaction_protocol'"
        ));
        assert!(sql.contains("configured CLI requires original owner creation ledger"));
        assert!(sql.contains("configured CLI has no native execution lease"));
        assert!(!sql.contains("UPDATE sdlc_tasks"));
        assert!(!sql.contains("INSERT INTO sdlc_pm_execution_leases"));
    }

    #[test]
    fn configured_completion_keeps_native_scope_clock_and_immutable_owner_tuple() {
        let sql = include_str!("configured_cli.sql");
        assert!(sql.contains("FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable()"));
        assert!(sql.contains("FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard()"));
        assert!(sql.contains("FOR EACH ROW EXECUTE FUNCTION tracker_sdlc_commit_clock()"));
        assert!(sql.contains("c.payload->>'owner_subject'=t.owner_subject"));
        assert!(
            sql.contains(
                "NEW.workflow_receipt->'assignment_version'=to_jsonb(e.assignment_version)"
            )
        );
        assert!(sql.contains(
            "pg_get_functiondef('sdlc_current_pm_completed(uuid,uuid,jsonb,text)'::regprocedure)"
        ));
        assert!(!sql.contains("DROP FUNCTION sdlc_current_pm_completed"));
        assert!(!sql.contains("INSERT INTO sdlc_analysis_intents"));
    }
}
