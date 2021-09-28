use crate::error::InsomniaError;

use anyhow::Result;
use serenity::{
    async_trait,
    client::Context,
    model::{
        channel::Message,
        id::{ChannelId, GuildId},
    },
    prelude::*,
};
use songbird::Call;
use std::sync::Arc;

#[async_trait]
pub trait CanJoinVoice {
    async fn join_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanJoinVoice for Message {
    async fn join_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>> {
        // Get guild ID and voice channel ID
        let manager = songbird::get(ctx).await.ok_or(InsomniaError::JoinVoice)?;
        let (guild_id, channel_id) = get_ids(ctx, self).await.ok_or(InsomniaError::JoinVoice)?;

        // Join voice channel
        let (handler_lock, error) = manager.join(guild_id, channel_id).await;
        if error.is_err() {
            return Err(InsomniaError::JoinVoice.into());
        }

        // Automatically deafen
        {
            let mut handler = handler_lock.lock().await;
            let _ = handler.deafen(true).await;
        }
        Ok(handler_lock)
    }
}

#[async_trait]
pub trait CanGetVoice {
    async fn get_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanGetVoice for Message {
    async fn get_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>> {
        // Get guild ID and voice channel ID
        let manager = songbird::get(ctx).await.ok_or(InsomniaError::GetVoice)?;
        let (guild_id, _) = get_ids(ctx, self).await.ok_or(InsomniaError::GetVoice)?;

        // Get voice channel
        Ok(manager.get_or_insert(guild_id.into()))
    }
}

async fn get_ids(ctx: &Context, msg: &Message) -> Option<(GuildId, ChannelId)> {
    let guild = msg.guild(&ctx.cache).await?;
    let guild_id = msg.guild_id?;
    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id)?;

    Some((guild_id, channel_id))
}
