use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::{async_trait, ChannelId};
use songbird::Call;

use super::error::MusicError;
use crate::PoiseContext;

#[async_trait]
pub trait CanJoinVoice {
    async fn join_voice(&self) -> Result<Arc<serenity::prelude::Mutex<Call>>, MusicError>;
}

#[async_trait]
impl CanJoinVoice for PoiseContext<'_> {
    async fn join_voice(&self) -> Result<Arc<serenity::prelude::Mutex<Call>>, MusicError> {
        let guild_id = self.guild_id().ok_or(MusicError::JoinVoice)?;
        let manager = songbird::get(self.serenity_context())
            .await
            .ok_or(MusicError::GetVoice)?;
        let channel_id = get_channel_id(self).await?;

        // Join voice channel
        let call = manager
            .join(guild_id, channel_id)
            .await
            .map_err(|_| MusicError::JoinVoice)?;

        // Automatically deafen
        let _ = call.lock().await.deafen(true).await;

        Ok(call)
    }
}

#[async_trait]
pub trait CanGetVoice {
    async fn get_voice(&self) -> Result<Arc<serenity::prelude::Mutex<Call>>, MusicError>;
}

#[async_trait]
impl CanGetVoice for PoiseContext<'_> {
    async fn get_voice(&self) -> Result<Arc<serenity::prelude::Mutex<Call>>, MusicError> {
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
