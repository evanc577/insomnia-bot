use serenity::{builder::CreateEmbed, http::Http, model::id::ChannelId};

use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};


pub enum SendMessage<'a> {
    Normal(&'a str),
    Error(&'a str),
    Custom(Box<dyn Fn(&mut CreateEmbed) + Sync + Send + 'a>),
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

