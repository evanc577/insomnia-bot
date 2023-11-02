use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, EntityTrait, QueryFilter, Schema,
};

use crate::config::Config;
use crate::{patchbot_forwarder, PoiseContext};

pub async fn create_table(config: &Config) -> String {
    let uri = format!(
        "postgres://{}:{}@{}:{}",
        config.database_user, config.database_password, config.database_host, config.database_port
    );
    let db = Database::connect(&uri).await.unwrap();

    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    let mut stmt = schema.create_table_from_entity(patchbot_forwarder::entity::Entity);
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

    let x = patchbot_forwarder::entity::ActiveModel {
        guild_id: Set(stringify(ctx.guild_id().map(|x| *x.as_u64()).unwrap_or(0))),
        source_channel_id: Set(stringify(source_channel_id)),
        match_text: Set(match_text.to_owned()),
        dest_channel_id: Set(stringify(dest_channel_id)),
        ..Default::default()
    };
    x.insert(&db).await.unwrap();
}

pub async fn delete_from_table(ctx: &PoiseContext<'_>, id: i64) -> u64 {
    let db_uri = ctx.data().db_uri.as_str();
    let db = Database::connect(db_uri).await.unwrap();

    let res = patchbot_forwarder::entity::Entity::delete_by_id(id)
        .filter(
            patchbot_forwarder::entity::Column::GuildId
                .eq(stringify(ctx.guild_id().map(|x| *x.as_u64()).unwrap_or(0))),
        )
        .exec(&db)
        .await
        .unwrap();
    res.rows_affected
}

pub async fn guild_rules(ctx: &PoiseContext<'_>) -> Vec<patchbot_forwarder::entity::Model> {
    let db_uri = ctx.data().db_uri.as_str();
    let db = Database::connect(db_uri).await.unwrap();

    patchbot_forwarder::entity::Entity::find()
        .filter(
            patchbot_forwarder::entity::Column::GuildId
                .eq(stringify(ctx.guild_id().map(|x| *x.as_u64()).unwrap_or(0))),
        )
        .all(&db)
        .await
        .unwrap()
}

fn stringify(x: u64) -> String {
    format!("{:x}", x)
}
