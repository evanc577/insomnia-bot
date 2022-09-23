use poise::{serenity_prelude as serenity, CreateReply, ReplyHandle};
use serenity::builder::CreateEmbed;
use serenity::http::Http;
use serenity::model::id::ChannelId;

use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};
use crate::PoiseContext;

pub const CANCEL_INTERACTION_ID: &str = "cancel";

pub enum SendMessage<'a> {
    Normal(&'a str),
    Error(&'a str),
    Custom(Box<dyn FnOnce(&mut CreateEmbed) + Send + 'a>),
    CustomCancelable(Box<dyn FnOnce(&mut CreateEmbed) + Send + 'a>),
}

/// Send a regular message to a channel
pub async fn send_msg_http(http: &Http, channel_id: ChannelId, message: SendMessage<'_>) {
    channel_id
        .send_message(http, |m| m.embed(|e| build_embed(e, message)))
        .await
        .unwrap();
}

/// Send a reply to a command
pub async fn send_msg<'a>(ctx: PoiseContext<'a>, message: SendMessage<'a>) -> ReplyHandle<'a> {
    ctx.send(|m| build_message(m, message)).await.unwrap()
}

/// Edit reply
pub async fn edit_reply(
    ctx: PoiseContext<'_>,
    reply_handle: ReplyHandle<'_>,
    message: SendMessage<'_>,
) {
    reply_handle
        .edit(ctx, |m| build_message(m, message))
        .await
        .unwrap();
}

fn build_message<'a, 'b, 'c>(
    m: &'a mut CreateReply<'b>,
    message: SendMessage<'c>,
) -> &'a mut CreateReply<'b> {
    // Set ephemeral on errors
    m.ephemeral(matches!(message, SendMessage::Error(_)));

    // Add cancel button if needed
    if matches!(message, SendMessage::CustomCancelable(_)) {
        m.components(|c| {
            c.create_action_row(|ar| {
                ar.create_button(|b| {
                    b.style(serenity::ButtonStyle::Danger)
                        .label("Cancel")
                        .custom_id(CANCEL_INTERACTION_ID)
                })
            })
        });
    } else {
        m.components(|c| c);
    }

    // Add embed
    m.embed(|e| build_embed(e, message))
}

fn build_embed<'a, 'b>(e: &'a mut CreateEmbed, message: SendMessage<'b>) -> &'a mut CreateEmbed {
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
        SendMessage::CustomCancelable(f) => f(e),
    }
    e
}
