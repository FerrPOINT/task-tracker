use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(&include_str!("untimed_delivery_binding.sql").replace("\r\n", "\n"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "untimed delivery binding is forward-only; retained history is unchanged".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untimed_binding_is_the_same_forward_tail_in_both_registered_lineages() {
        for migrations in [
            crate::PublishedMigrator::migrations(),
            crate::ProjectRepositoryMigrator::migrations(),
        ] {
            assert_eq!(migrations.last().unwrap().name(), Migration.name());
            assert_eq!(
                migrations
                    .iter()
                    .filter(|m| m.name() == Migration.name())
                    .count(),
                1
            );
            assert_eq!(
                migrations[migrations.len() - 2].name(),
                "m20261107_0000115_agent_stage_status"
            );
        }
    }
}
