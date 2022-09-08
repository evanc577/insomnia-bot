use serenity::builder::CreateEmbed;
use serenity::http::Http;
use serenity::model::id::ChannelId;

use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};
use crate::PoiseContext;

pub enum SendMessage<'a> {
    Normal(&'a str),
    Error(&'a str),
    Custom(Box<dyn FnOnce(&mut CreateEmbed) + Send + 'a>),
}

pub async fn send_msg_http(http: &Http, channel_id: ChannelId, message: SendMessage<'_>) {
    channel_id
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
        .await
        .unwrap();
}

pub async fn send_msg(ctx: PoiseContext<'_>, message: SendMessage<'_>) {
    ctx.send(|m| {
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
    .await
    .unwrap();
}
