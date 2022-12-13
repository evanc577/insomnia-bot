use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::{async_trait, ChannelId, Mutex};
use songbird::Call;

use super::error::MusicError;
use crate::PoiseContext;

#[async_trait]
pub trait CanJoinVoice {
    async fn join_voice(&self) -> Result<Arc<Mutex<Call>>, MusicError>;
}

#[async_trait]
impl CanJoinVoice for PoiseContext<'_> {
    async fn join_voice(&self) -> Result<Arc<Mutex<Call>>, MusicError> {
        let guild_id = self.guild_id().ok_or(MusicError::JoinVoice)?;
        let manager = songbird::get(self.serenity_context())
            .await
            .ok_or(MusicError::GetVoice)?;
        let channel_id = get_channel_id(self).await?;

        // Join voice channel
        let (handler_lock, error) = manager.join(guild_id, channel_id).await;
        if error.is_err() {
            let _ = dbg!(error);
            let _ = manager.leave(guild_id).await;
            return Err(MusicError::JoinVoice);
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
    async fn get_voice(&self) -> Result<Arc<Mutex<Call>>, MusicError>;
}

#[async_trait]
impl CanGetVoice for PoiseContext<'_> {
    async fn get_voice(&self) -> Result<Arc<Mutex<Call>>, MusicError> {
        get_channel_id(self).await?;
        let guild_id = self.guild_id().ok_or(MusicError::GetVoice)?;
        let manager = songbird::get(self.serenity_context())
            .await
            .ok_or(MusicError::GetVoice)?;
        Ok(manager.get_or_insert(guild_id))
    }
}

async fn get_channel_id(ctx: &PoiseContext<'_>) -> Result<ChannelId, MusicError> {
    let channel_id = ctx
        .guild()
        .ok_or(MusicError::GetVoice)?
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id)
        .ok_or(MusicError::NotInVoiceChannel)?;

    Ok(channel_id)
}
