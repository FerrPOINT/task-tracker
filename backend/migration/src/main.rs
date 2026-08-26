use sea_orm_migration::MigratorTrait;

#[async_std::main]
async fn main() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "up".to_string());

    let db = sea_orm::Database::connect(&url)
        .await
        .expect("failed to connect to database");

    match cmd.as_str() {
        "up" => {
            migration::Migrator::up(&db, None)
                .await
                .expect("migration up failed");
            println!("migrations applied");
        }
        "status" => {
            let status = migration::Migrator::get_applied_migrations(&db)
                .await
                .expect("failed to get applied migrations");
            println!("applied {} migrations:", status.len());
            for m in status {
                println!("  {}", m.name());
            }
        }
        "fresh" => {
            migration::Migrator::fresh(&db)
                .await
                .expect("migration fresh failed");
            println!("database recreated, migrations applied");
        }
        "down" => {
            migration::Migrator::down(&db, Some(1))
                .await
                .expect("migration down failed");
            println!("last migration reverted");
        }
        other => panic!("unknown command: {other} (use up|status|fresh|down)"),
    }
}
