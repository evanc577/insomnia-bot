use std::sync::Arc;

use poise::serenity_prelude::{ChannelId, CreateEmbed, Http, Message};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

use super::{entity::*, error::PatchbotForwardError};

pub async fn forward(db_uri: &str, http: Arc<Http>, message: Message) -> Result<(), PatchbotForwardError>{
    let embed_author = message
        .embeds
        .iter()
        .find(|e| e.author.is_some())
        .ok_or(PatchbotForwardError::InvalidEmbeds)?
        .author
        .clone()
        .ok_or(PatchbotForwardError::InvalidEmbeds)?
        .name;

    let db = Database::connect(db_uri).await.unwrap();

    let matches = Entity::find()
        .filter(Column::SourceChannelId.eq(format!("{:x}", message.channel_id.as_u64())))
        .filter(Column::MatchText.eq(embed_author))
        .all(&db)
        .await
        .unwrap();

    for x in matches {
        let dest_channel_id = ChannelId(u64::from_str_radix(&x.dest_channel_id, 16).unwrap());
        eprintln!("Forwarding message to {}", dest_channel_id.as_u64());
        dest_channel_id
            .send_message(http.clone(), |f| {
                f.set_embeds(
                    message
                        .embeds
                        .iter()
                        .map(|e| CreateEmbed::from(e.clone()))
                        .collect::<Vec<_>>(),
                )
            })
            .await
            .unwrap();
    }

    Ok(())
}
