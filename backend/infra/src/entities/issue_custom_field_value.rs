use sea_orm::entity::prelude::*;
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "issue_custom_field_values")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub issue_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub field_id: Uuid,
    pub value: JsonValue,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
