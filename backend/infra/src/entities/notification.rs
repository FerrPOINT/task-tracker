use sea_orm::entity::prelude::*;
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "notifications")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub recipient_id: Uuid,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub title: String,
    pub body: Option<String>,
    pub is_read: bool,
    pub read_at: Option<DateTimeWithTimeZone>,
    pub action_url: Option<String>,
    pub metadata: JsonValue,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::RecipientId",
        to = "super::user::Column::Id"
    )]
    Recipient,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::ActorId",
        to = "super::user::Column::Id"
    )]
    Actor,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Recipient.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
