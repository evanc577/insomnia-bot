use std::sync::Arc;

use poise::serenity_prelude::{ChannelId, CreateEmbed, Http, Message};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

use crate::CLIENT;

use super::{entity::*, error::PatchbotForwardError};

pub async fn forward(
    db_uri: &str,
    http: Arc<Http>,
    message: Message,
) -> Result<(), PatchbotForwardError> {
    // Extract embed
    let embed = message
        .embeds
        .iter()
        .find(|e| {
            if let Some(author) = &e.author {
                !author
                    .name
                    .to_lowercase()
                    .contains("this update is brought to you by")
            } else {
                false
            }
        });
    if embed.is_none() {
        return Ok(());
    };
    let embed = embed.unwrap();
    let embed_author = embed
        .author
        .clone()
        .ok_or(PatchbotForwardError::InvalidEmbeds)?
        .name;

    // Find matching forward rules
    let db = Database::connect(db_uri).await.unwrap();
    let matches = Entity::find()
        .filter(Column::GuildId.eq(format!(
                "{:x}",
                message
                    .guild_id.map(|x| *x.as_u64())
                    .unwrap_or(0)
            )))
        .filter(Column::SourceChannelId.eq(format!("{:x}", message.channel_id.as_u64())))
        .filter(Column::MatchText.eq(embed_author))
        .all(&db)
        .await
        .unwrap();

    // Replace Patchbot's tracking URL with the final redirect
    let redirected_url: Option<String> = if let Some(ref url) = embed.url {
        if let Ok(resp) = CLIENT.get(url).send().await {
            Some(resp.url().as_str().to_owned())
        } else {
            Some(url.clone())
        }
    } else {
        None
    };
    let mut embed = embed.clone();
    embed.url = redirected_url;

    // Forward messages
    for x in matches {
        let dest_channel_id = ChannelId(u64::from_str_radix(&x.dest_channel_id, 16).unwrap());
        eprintln!(
            "Forwarding Patchbot message to {}",
            dest_channel_id.as_u64()
        );
        dest_channel_id
            .send_message(http.clone(), |f| {
                f.set_embed(CreateEmbed::from(embed.clone()))
            })
            .await
            .unwrap();
    }

    Ok(())
}
