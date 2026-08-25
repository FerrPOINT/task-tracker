use domain::Repositories;
use migration::MigratorTrait;
use sea_orm::{ConnectOptions, Database};
use shared::{AppError, DatabaseConfig};

use crate::repos::SeaOrmRepositories;

pub async fn build_repositories(config: DatabaseConfig) -> Result<Repositories, AppError> {
    let mut opt = ConnectOptions::new(config.url);
    opt.max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(std::time::Duration::from_secs(5));
    let db = Database::connect(opt).await.map_err(AppError::database)?;
    let repos = SeaOrmRepositories::new(db);

    Ok(Repositories {
        users: repos.users,
        audit_logs: repos.audit_logs,
        system_settings: repos.system_settings,
        projects: repos.projects,
        issues: repos.issues,
        boards: repos.boards,
        sprints: repos.sprints,
        comments: repos.comments,
        worklogs: repos.worklogs,
        members: repos.members,
        statuses: repos.statuses,
        transitions: repos.transitions,
        issue_types: repos.issue_types,
        attachments: repos.attachments,
        labels: repos.labels,
        issue_links: repos.issue_links,
        notifications: repos.notifications,
        notification_settings: repos.notification_settings,
        issue_status_history: repos.issue_status_history,
        watchers: repos.watchers,
        votes: repos.votes,
        components: repos.components,
        versions: repos.versions,
        custom_fields: repos.custom_fields,
    })
}

pub async fn run_migrations(config: DatabaseConfig) -> Result<(), AppError> {
    let mut opt = ConnectOptions::new(config.url);
    opt.max_connections(1);
    let db = Database::connect(opt).await.map_err(AppError::database)?;
    migration::Migrator::up(&db, None)
        .await
        .map_err(AppError::database)?;
    Ok(())
}
