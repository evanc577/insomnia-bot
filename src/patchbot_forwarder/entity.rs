use sea_orm::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "forwarder")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub guild_id: String,
    pub source_channel_id: String,
    pub match_text: String,
    pub dest_channel_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
