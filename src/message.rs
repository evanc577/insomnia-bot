use std::fmt::Display;
use std::sync::LazyLock;

use poise::serenity_prelude::CreateMessage;
use poise::{async_trait, serenity_prelude as serenity, CreateReply, ReplyHandle};
use serenity::builder::CreateEmbed;
use serenity::http::Http;
use serenity::model::id::ChannelId;
use serenity::Color;

use crate::PoiseContext;

pub const CANCEL_INTERACTION_ID: &str = "cancel";
pub static EMBED_COLOR: LazyLock<Color> = LazyLock::new(|| Color::from_rgb(0x10, 0x18, 0x20));
pub static EMBED_PLAYING_COLOR: LazyLock<Color> =
    LazyLock::new(|| Color::from_rgb(0x77, 0xDD, 0x77));
pub static EMBED_ERROR_COLOR: LazyLock<Color> = LazyLock::new(|| Color::from_rgb(0x8a, 0x2a, 0x2b));

pub enum SendMessage<T>
where
    T: Display,
{
    Normal(T),
    Error(T),
}

pub enum CustomSendMessage<'a> {
    Custom(Box<dyn FnOnce() -> CreateEmbed + Send + Sync + 'a>),
    Cancelable(Box<dyn FnOnce() -> CreateEmbed + Send + Sync + 'a>),
}

#[async_trait]
pub trait SendableMessage {
    /// Send a regular message to a channel
    async fn send_msg_http(self, http: &Http, channel_id: ChannelId)
    where
        Self: Sized,
    {
        channel_id
            .send_message(http, CreateMessage::new().embed(self.build_embed()))
            .await
            .unwrap();
    }

    /// Send a reply to a command
    async fn send_msg(self, ctx: PoiseContext<'_>) -> ReplyHandle<'_>
    where
        Self: Sized,
    {
        ctx.send(|m| self.build_message(m)).await.unwrap()
    }

    /// Edit reply
    async fn edit_reply(self, ctx: PoiseContext<'_>, reply_handle: ReplyHandle<'_>)
    where
        Self: Sized,
    {
        reply_handle
            .edit(ctx, |m| self.build_message(m))
            .await
            .unwrap();
    }

    fn build_message(self) -> CreateReply
    where
        Self: Sized,
    {
        let m = CreateMessage::new();
        // Add cancel button if needed
        if self.is_cancelable() {
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

        // Set ephemeral on errors
        m.ephemeral(self.is_ephemeral());

        // Add embed
        m.embed(|e| self.build_embed(e))
    }

    fn build_embed(self) -> CreateEmbed;
    fn is_cancelable(&self) -> bool;
    fn is_ephemeral(&self) -> bool;
}

impl<T> SendableMessage for SendMessage<T>
where
    T: Display,
{
    fn build_embed(self) -> CreateEmbed {
        match self {
            Self::Normal(s) => {
                let s = to_string_or_default(s);
                CreateEmbed::new().description(s).color(*EMBED_COLOR)
            }
            Self::Error(s) => {
                let mut s = to_string_or_default(s);
                if let Some(c) = s.get_mut(0..1) {
                    c.make_ascii_uppercase();
                }
                CreateEmbed::new().title("Error").description(s).color(*EMBED_ERROR_COLOR)
            }
        }
    }

    fn is_cancelable(&self) -> bool {
        false
    }

    fn is_ephemeral(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

impl<'a> SendableMessage for CustomSendMessage<'a> {
    fn build_embed(self) -> CreateEmbed {
        match self {
            Self::Custom(f) => f(),
            Self::Cancelable(f) => f(),
        }
        
    }

    fn is_cancelable(&self) -> bool {
        matches!(self, Self::Cancelable(_))
    }

    fn is_ephemeral(&self) -> bool {
        false
    }
}

fn to_string_or_default(val: impl Display) -> String {
    let s = val.to_string();
    if s.is_empty() {
        String::from("no description")
    } else {
        s
    }
}
