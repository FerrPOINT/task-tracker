use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("project_repository_links").await? {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"CREATE TABLE project_repository_links (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    forge_instance_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    snapshot jsonb NOT NULL,
    created_by uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id,forge_instance_id,repository_id)
);
CREATE INDEX project_repository_links_project_idx
    ON project_repository_links(project_id,created_at,repository_id);

CREATE TRIGGER project_repository_links_namespace_write_guard
BEFORE INSERT OR UPDATE OR DELETE ON project_repository_links
FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard();
"#,
                )
                .await?;
        }
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "project repository associations must be preserved".into(),
        ))
    }
}
