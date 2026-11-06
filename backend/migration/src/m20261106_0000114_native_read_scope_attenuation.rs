use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("native_read_scope_attenuation.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "native scope attenuation is forward-only".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    #[tokio::test]
    #[ignore = "requires a fresh owned *_native_scope_test PostgreSQL database"]
    async fn retained_reads_attenuate_mutation_without_granting_writes() {
        let url = std::env::var("TT_NATIVE_SCOPE_TEST_DATABASE_URL")
            .expect("disposable native scope database required");
        let db = Database::connect(url).await.unwrap();
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT current_database() AS name,to_regclass('public.projects') IS NULL AS fresh"
                    .to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(
            row.try_get::<String>("", "name")
                .unwrap()
                .ends_with("_native_scope_test")
        );
        assert!(row.try_get::<bool>("", "fresh").unwrap());
        db.execute_unprepared(include_str!("native_scope_test_fixture.sql"))
            .await
            .unwrap();
        let original = include_str!("native_sdlc_spaces.sql");
        let prefix = original
            .split("CREATE OR REPLACE FUNCTION tracker_namespace_write_guard")
            .next()
            .unwrap();
        db.execute_unprepared(prefix).await.unwrap();
        let artifacts = include_str!("native_task_artifacts.sql")
            .split("CREATE OR REPLACE FUNCTION tracker_authorize_project_content")
            .next()
            .unwrap();
        db.execute_unprepared(artifacts).await.unwrap();
        db.execute_unprepared(include_str!("native_scope_test_red.sql"))
            .await
            .unwrap();
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        db.execute_unprepared(include_str!("native_scope_test_assertions.sql"))
            .await
            .unwrap();
    }
}
