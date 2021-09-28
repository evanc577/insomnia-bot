use crate::error::InsomniaError;

use anyhow::Result;
use serenity::{
    async_trait,
    client::Context,
    model::channel::Message,
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
        let manager = songbird::get(ctx)
            .await
            .ok_or(InsomniaError::JoinVoice)?
            .clone();

        let guild = self.guild(&ctx.cache).await.ok_or(InsomniaError::JoinVoice)?;
        let guild_id = self.guild_id.ok_or(InsomniaError::JoinVoice)?;
        let channel_id = guild
            .voice_states
            .get(&self.author.id)
            .and_then(|voice_state| voice_state.channel_id)
            .ok_or(InsomniaError::JoinVoice)?;

        let (handler_lock, error) = manager.join(guild_id, channel_id).await;
        if error.is_err() {
            return Err(InsomniaError::JoinVoice.into());
        }

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
        let manager = songbird::get(ctx)
            .await
            .ok_or(InsomniaError::GetVoice)?
            ;
        match self.guild_id {
            Some(id) => Ok(manager.get_or_insert(id.into())),
            None => return Err(InsomniaError::GetVoice.into()),
        }
    }
}
