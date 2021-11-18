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
use std::{collections::HashMap, sync::Arc};

pub struct CallMutexMap;

impl TypeMapKey for CallMutexMap {
    type Value = HashMap<GuildId, Arc<Mutex<()>>>;
}

#[async_trait]
pub trait CanJoinVoice {
    async fn join_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanJoinVoice for Message {
    async fn join_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>> {
        let mutex = get_lock(ctx, self)
            .await
            .map_err(|_| InsomniaError::JoinVoice)?;
        let _lock = mutex.lock().await;

        // Get guild ID and voice channel ID
        let manager = songbird::get(ctx).await.ok_or(InsomniaError::JoinVoice)?;
        let (guild_id, channel_id) = get_ids(ctx, self).await.ok_or(InsomniaError::JoinVoice)?;

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
    async fn get_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>>;
}

#[async_trait]
impl CanGetVoice for Message {
    async fn get_voice(&self, ctx: &Context) -> Result<Arc<Mutex<Call>>> {
        let mutex = get_lock(ctx, self)
            .await
            .map_err(|_| InsomniaError::GetVoice)?;
        let _lock = mutex.lock().await;

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

async fn get_lock(ctx: &Context, msg: &Message) -> Result<Arc<Mutex<()>>> {
    let (guild_id, _) = get_ids(ctx, msg).await.ok_or(InsomniaError::VoiceLock)?;
    let data = ctx.data.read().await;
    let map = data.get::<CallMutexMap>().ok_or(InsomniaError::VoiceLock)?;
    let m = match map.get(&guild_id) {
        Some(m) => m.clone(),
        None => {
            let m = Arc::new(Mutex::new(()));
            drop(data);
            let mut data = ctx.data.write().await;
            let map = data
                .get_mut::<CallMutexMap>()
                .ok_or(InsomniaError::VoiceLock)?;
            map.insert(guild_id, m.clone());
            m
        }
    };

    Ok(m)
}
