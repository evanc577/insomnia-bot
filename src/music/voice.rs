use std::sync::Arc;

use anyhow::Result;
use poise::serenity_prelude::*;
use songbird::Call;

use super::error::MusicError;
use crate::error::InsomniaError;
use crate::message::{send_msg, SendMessage};
use crate::PoiseContext;

#[async_trait]
pub trait CanJoinVoice {
    async fn join_voice(&self) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanJoinVoice for PoiseContext<'_> {
    async fn join_voice(&self) -> Result<Arc<Mutex<Call>>> {
        let guild_id = self.guild_id().ok_or(InsomniaError::JoinVoice)?;
        let manager = songbird::get(self.discord())
            .await
            .ok_or(InsomniaError::GetVoice)?;
        let channel_id = get_channel_id(self).await.ok_or(InsomniaError::JoinVoice)?;

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
    async fn in_voice_and_send_msg(&self) -> bool;
}

#[async_trait]
impl CanGetVoice for PoiseContext<'_> {
    async fn get_voice(&self) -> Result<Arc<Mutex<Call>>> {
        get_channel_id(self).await.ok_or(InsomniaError::GetVoice)?;
        let guild_id = self.guild_id().ok_or(InsomniaError::GetVoice)?;
        let manager = songbird::get(self.discord())
            .await
            .ok_or(InsomniaError::GetVoice)?;
        Ok(manager.get_or_insert(guild_id))
    }

    async fn in_voice_and_send_msg(&self) -> bool {
        match get_channel_id(self).await {
            Some(_) => true,
            None => {
                send_msg(
                    *self,
                    SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
                )
                .await;
                false
            }
        }
    }
}

async fn get_channel_id(ctx: &PoiseContext<'_>) -> Option<ChannelId> {
    let channel_id = ctx
        .guild()?
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id)?;

    Some(channel_id)
}
