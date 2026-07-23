use sea_orm::entity::prelude::*;
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "issues")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub key: String,
    #[sea_orm(column_type = "String(StringLen::N(16))")]
    pub issue_type: String,
    pub status_id: Uuid,
    pub summary: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub reporter_id: Uuid,
    #[sea_orm(column_type = "String(StringLen::N(16))")]
    pub priority: String,
    pub labels: JsonValue,
    pub sprint_id: Option<Uuid>,
    pub position: f64,
    pub due_date: Option<DateTimeWithTimeZone>,
    pub original_estimate_seconds: Option<i64>,
    pub remaining_estimate_seconds: Option<i64>,
    pub time_spent_seconds: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
