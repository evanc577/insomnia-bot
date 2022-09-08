use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use poise::serenity_prelude::{self as serenity, *};
use songbird::Call;

use crate::error::InsomniaError;
use crate::PoiseContext;

pub struct CallMutexMap;

impl serenity::TypeMapKey for CallMutexMap {
    type Value = HashMap<GuildId, Arc<Mutex<()>>>;
}

#[async_trait]
pub trait CanJoinVoice {
    async fn join_voice(&self) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanJoinVoice for PoiseContext<'_> {
    async fn join_voice(&self) -> Result<Arc<Mutex<Call>>> {
        let guild_id = self.guild_id().ok_or(InsomniaError::GetVoice)?;
        let manager = songbird::get(self.discord())
            .await
            .ok_or(InsomniaError::GetVoice)?;
        let channel_id = get_channel_id(self).await.unwrap();

        // Join voice channel
        let (handler_lock, error) = manager.join(guild_id, channel_id).await;
        if error.is_err() {
            let _ = dbg!(error);
            let _ = manager.leave(guild_id).await;
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
    async fn get_voice(&self) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanGetVoice for PoiseContext<'_> {
    async fn get_voice(&self) -> Result<Arc<Mutex<Call>>> {
        let guild_id = self.guild_id().ok_or(InsomniaError::GetVoice)?;
        let manager = songbird::get(self.discord())
            .await
            .ok_or(InsomniaError::GetVoice)?;
        Ok(manager.get_or_insert(guild_id))
    }
}

async fn get_channel_id(ctx: &PoiseContext<'_>) -> Option<ChannelId> {
    let channel_id = ctx
        .guild()
        .unwrap()
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id)?;

    Some(channel_id)
}
