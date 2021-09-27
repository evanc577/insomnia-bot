use std::sync::Arc;

use serenity::{builder::CreateEmbed, http::Http, model::id::ChannelId};
use songbird::tracks::TrackHandle;

use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};

pub fn format_track(track: &TrackHandle, format: bool) -> String {
    let title = track.metadata().title.clone().unwrap_or("Unknown".into());

    if format {
        format!(
            "**{}**",
            title
                .replace("*", "\\*")
                .replace("_", "\\_")
                .replace("~", "\\~")
                .replace("`", "")
        )
    } else {
        title
    }
}

pub enum SendMessage<'a> {
    Normal(&'a str),
    Error(&'a str),
    Custom(Arc<dyn Fn(&mut CreateEmbed) + Sync + Send>),
}

pub async fn send_msg(http: &Http, channel_id: ChannelId, message: SendMessage<'_>) {
    let _ = channel_id
        .send_message(http, |m| {
            m.embed(|e| {
                match message {
                    SendMessage::Normal(s) => {
                        e.description(s);
                        e.color(*EMBED_COLOR);
                    }
                    SendMessage::Error(s) => {
                        e.description(s);
                        e.color(*EMBED_ERROR_COLOR);
                    }
                    SendMessage::Custom(f) => f(e),
                }
                e
            })
        })
        .await;
}

