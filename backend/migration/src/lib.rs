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
pub mod m20260827_0000028_core_fk_constraints;
pub mod m20260831_0000029_notification_digest_state;
pub mod m20260914_0000030_totp;
mod m20260914_0000031_password_reset;
mod m20260914_0000032_oidc;
mod m20260918_0000033_central_subject;
pub mod m20261001_0000034_sdlc_clarification;
mod m20261008_0000090_namespace;
mod m20261010_0000091_pm_native_admission;
mod m20261010_0000092_pm_state_write_permits;
mod m20261012_0000093_fleet_mcp_admission;
mod m20261012_0000094_analysis_result;
mod m20261012_0000095_pm_completion;
mod m20261013_0000096_single_task_delivery;
mod m20261017_0000097_task_source_binding;
mod m20261018_0000098_pm_unsubmitted_recovery;
mod m20261021_0000099_pm_bootstrap_retry;
mod m20261023_0000103_pm_settled_input_recovery;
mod m20261026_0000104_pm_manual_recovery_budget;
mod m20261026_0000105_configured_cli;
pub mod m20261027_0000106_configured_stage_comments;
pub mod m20261027_0000107_configured_business_history;
mod m20261027_0000108_configured_critical_lifecycle;
mod m20261028_0000109_configured_initial_adoption;
mod m20261028_0000110_configured_checkpoint_absence;
mod m20261029_0000111_configured_analysis_start;

pub use sea_orm_migration::prelude::*;

mod m20261016_0000096_project_repositories;
mod m20261016_0000096_task_source_compatibility;
mod m20261018_0000098_project_repository_catalog;
mod m20261019_0000099_unified_spaces;
mod m20261021_0000100_human_sdlc_spaces;
mod m20261021_0000101_native_sdlc_spaces;
mod m20261022_0000102_request_commit_clock;
mod m20261022_0000103_identity_deletion;

pub struct Migrator;
struct PublishedMigrator;
struct ProjectRepositoryMigrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        PublishedMigrator::migrations()
    }

    async fn get_migration_with_status<C>(
        db: &C,
    ) -> Result<Vec<sea_orm_migration::Migration>, DbErr>
    where
        C: ConnectionTrait,
    {
        let history = Self::get_migration_models(db).await?;
        if history
            .iter()
            .any(|m| m.version == "m20261016_0000096_project_repositories")
        {
            ProjectRepositoryMigrator::get_migration_with_status(db).await
        } else {
            PublishedMigrator::get_migration_with_status(db).await
        }
    }
}

#[async_trait::async_trait]
impl MigratorTrait for ProjectRepositoryMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = PublishedMigrator::migrations();
        let index = migrations
            .iter()
            .position(|m| m.name() == "m20261017_0000097_task_source_binding")
            .expect("source binding migration registered");
        migrations.insert(
            index,
            Box::new(m20261016_0000096_project_repositories::Migration),
        );
        migrations.insert(
            index + 1,
            Box::new(m20261016_0000096_task_source_compatibility::Migration),
        );
        migrations
    }
}

#[async_trait::async_trait]
impl MigratorTrait for PublishedMigrator {
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
            Box::new(m20260827_0000028_core_fk_constraints::Migration),
            Box::new(m20260831_0000029_notification_digest_state::Migration),
            Box::new(m20260914_0000030_totp::Migration),
            Box::new(m20260914_0000031_password_reset::Migration),
            Box::new(m20260914_0000032_oidc::Migration),
            Box::new(m20260918_0000033_central_subject::Migration),
            Box::new(m20261001_0000034_sdlc_clarification::Migration),
            Box::new(m20261008_0000090_namespace::Migration),
            Box::new(m20261010_0000091_pm_native_admission::Migration),
            Box::new(m20261010_0000092_pm_state_write_permits::Migration),
            Box::new(m20261012_0000093_fleet_mcp_admission::Migration),
            Box::new(m20261012_0000094_analysis_result::Migration),
            Box::new(m20261012_0000095_pm_completion::Migration),
            Box::new(m20261013_0000096_single_task_delivery::Migration),
            Box::new(m20261017_0000097_task_source_binding::Migration),
            Box::new(m20261018_0000098_pm_unsubmitted_recovery::Migration),
            Box::new(m20261018_0000098_project_repository_catalog::Migration),
            Box::new(m20261021_0000099_pm_bootstrap_retry::Migration),
            Box::new(m20261019_0000099_unified_spaces::Migration),
            Box::new(m20261021_0000100_human_sdlc_spaces::Migration),
            Box::new(m20261021_0000101_native_sdlc_spaces::Migration),
            Box::new(m20261022_0000102_request_commit_clock::Migration),
            Box::new(m20261022_0000103_identity_deletion::Migration),
            Box::new(m20261023_0000103_pm_settled_input_recovery::Migration),
            Box::new(m20261026_0000104_pm_manual_recovery_budget::Migration),
            Box::new(m20261026_0000105_configured_cli::Migration),
            Box::new(m20261027_0000106_configured_stage_comments::Migration),
            Box::new(m20261027_0000107_configured_business_history::Migration),
            Box::new(m20261027_0000108_configured_critical_lifecycle::Migration),
            Box::new(m20261028_0000109_configured_initial_adoption::Migration),
            Box::new(m20261028_0000110_configured_checkpoint_absence::Migration),
            Box::new(m20261029_0000111_configured_analysis_start::Migration),
        ]
    }
}

#[cfg(test)]
mod project_repository_tests;

#[cfg(test)]
mod unified_space_tests;
