use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP_SQL: &str = r#"
-- Repository associations belong to the project, not an individual task.
-- Existing task links are consolidated under their issue's owning project so
-- every project has one authoritative repository collection after this upgrade.
CREATE TABLE project_repository_links (
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

-- Preserve prior task-scoped records under their owning project, then remove
-- the obsolete task-scoped relation so it cannot remain a competing model.
INSERT INTO project_repository_links(project_id,forge_instance_id,repository_id,snapshot,created_by,created_at)
SELECT i.project_id,l.forge_instance_id,l.repository_id,l.snapshot,l.created_by,l.created_at
FROM task_repository_links l JOIN issues i ON i.id=l.issue_id
ON CONFLICT(project_id,forge_instance_id,repository_id) DO NOTHING;
DROP TABLE task_repository_links;

CREATE TRIGGER project_repository_links_namespace_write_guard
BEFORE INSERT OR UPDATE OR DELETE ON project_repository_links
FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP_SQL).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "project repository links retain audit-relevant project associations; destructive down is disabled"
                .into(),
        ))
    }
}
