pub mod m20250723_000001_create_tables;
pub mod m20250723_000002_seed_demo_data;

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250723_000001_create_tables::Migration),
            Box::new(m20250723_000002_seed_demo_data::Migration),
        ]
    }
}
