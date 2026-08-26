pub mod m20250723_0000015_workflow_and_issue_types;
pub mod m20250723_0000016_labels;
pub mod m20250723_0000017_issue_links;
pub mod m20250723_0000018_fulltext_search;
pub mod m20250723_000001_create_tables;
pub mod m20260824_0000020_notifications;
pub mod m20260824_0000021_admin_audit_settings;
pub mod m20260824_0000022_performance_indexes;
pub mod m20260825_0000023_watchers_votes;
pub mod m20260825_0000024_issue_soft_delete;
pub mod m20260825_0000025_components_versions;
pub mod m20260825_0000026_custom_fields;
pub mod m20260826_0000027_fk_indexes;

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250723_000001_create_tables::Migration),
            Box::new(m20250723_0000015_workflow_and_issue_types::Migration),
            Box::new(m20250723_0000016_labels::Migration),
            Box::new(m20250723_0000017_issue_links::Migration),
            Box::new(m20250723_0000018_fulltext_search::Migration),
            Box::new(m20260824_0000020_notifications::Migration),
            Box::new(m20260824_0000021_admin_audit_settings::Migration),
            Box::new(m20260824_0000022_performance_indexes::Migration),
            Box::new(m20260825_0000023_watchers_votes::Migration),
            Box::new(m20260825_0000024_issue_soft_delete::Migration),
            Box::new(m20260825_0000025_components_versions::Migration),
            Box::new(m20260825_0000026_custom_fields::Migration),
            Box::new(m20260826_0000027_fk_indexes::Migration),
        ]
    }
}
