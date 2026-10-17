use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("task_source_binding.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "task source history is immutable; destructive rollback is disabled".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_migration_is_additive_and_never_enrolls_historical_tasks() {
        let sql = include_str!("task_source_binding.sql");
        assert!(!sql.contains("ALTER TABLE"));
        assert!(!sql.contains("UPDATE sdlc_tasks"));
        assert!(!sql.contains("CREATE OR REPLACE"));
        for name in [
            "sdlc_source_policy_revisions",
            "sdlc_source_policy_operations",
            "sdlc_source_ref_owners",
            "sdlc_task_source_bindings",
            "sdlc_draft_source_enrollments",
            "sdlc_effective_task_source_selections",
        ] {
            assert!(sql.contains(name), "{name}");
        }
        assert!(sql.contains("EXECUTE FUNCTION sdlc_history_immutable()"));
        assert!(sql.contains("EXECUTE FUNCTION tracker_namespace_write_guard()"));
        assert!(sql.contains("REFERENCES sdlc_confirmations"));
        assert!(sql.contains("REFERENCES task_repository_links"));
        assert!(sql.contains("sdlc_draft_source_enrollment_gate BEFORE INSERT"));
        assert!(sql.contains("t.state->>'stage'='Draft'"));
        assert!(
            sql.contains("OR EXISTS(SELECT 1 FROM sdlc_confirmations WHERE task_id=NEW.task_id)")
        );
        assert!(sql.contains("FROM sdlc_delivery_results WHERE task_id=NEW.task_id"));
        assert!(!sql.contains("INSERT INTO sdlc_draft_source_enrollments"));
        assert!(!sql.contains("runtime_observation"));
        assert!(!sql.contains("last_native"));
    }
}
