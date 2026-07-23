use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sprints")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub goal: Option<String>,
    #[sea_orm(column_type = "String(StringLen::N(16))")]
    pub state: String,
    pub start_date: Option<DateTimeWithTimeZone>,
    pub end_date: Option<DateTimeWithTimeZone>,
    pub velocity: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
