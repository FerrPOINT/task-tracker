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
}
