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
        projects: repos.projects,
        issues: repos.issues,
        boards: repos.boards,
        sprints: repos.sprints,
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
