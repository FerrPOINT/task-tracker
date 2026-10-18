use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE task_repository_links (
    issue_id uuid NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    forge_instance_id uuid NOT NULL, repository_id uuid NOT NULL, snapshot jsonb NOT NULL,
    created_by uuid NOT NULL REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(issue_id,forge_instance_id,repository_id)
);
CREATE TRIGGER namespace_write_guard BEFORE INSERT OR UPDATE OR DELETE ON task_repository_links
FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard();
"#,
            )
            .await?;
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "task source snapshots must be preserved".into(),
        ))
    }
}
