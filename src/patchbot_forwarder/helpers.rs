use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, Database, Schema};

use super::entity::*;
use crate::config::Config;
use crate::PoiseContext;

pub async fn create_table(config: &Config) -> String {
    let uri = format!(
        "postgres://{}:{}@{}:{}",
        config.database_user, config.database_password, config.database_host, config.database_port
    );
    let db = Database::connect(&uri).await.unwrap();

    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    let mut stmt = schema.create_table_from_entity(Entity);
    stmt.if_not_exists();
    db.execute(backend.build(&stmt)).await.unwrap();

    uri
}

pub async fn insert_to_table(
    ctx: &PoiseContext<'_>,
    match_text: &str,
    source_channel_id: u64,
    dest_channel_id: u64,
) {
    let db_uri = ctx.data().db_uri.as_str();
    let db = Database::connect(db_uri).await.unwrap();

    let x = ActiveModel {
        guild_id: Set(format!(
            "{:x}",
            ctx.guild_id().and_then(|x| Some(*x.as_u64())).unwrap_or(0)
        )),
        source_channel_id: Set(format!("{:x}", source_channel_id)),
        match_text: Set(match_text.to_owned()),
        dest_channel_id: Set(format!("{:x}", dest_channel_id)),
        ..Default::default()
    };
    x.insert(&db).await.unwrap();
}
