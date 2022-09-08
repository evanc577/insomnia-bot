use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::stream::StreamExt;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use songbird::input::{self, Restartable};
use songbird::tracks::{Track, TrackHandle};
use songbird::{Event, TrackEvent};

use super::error::MusicError;
use super::events::{TrackEndNotifier, TrackSegmentSkipper, TrackStartNotifier};
use super::message::{format_update, PlayUpdate};
use super::voice::{CanGetVoice, CanJoinVoice};
use crate::error::InsomniaError;
use crate::message::{send_msg, SendMessage};
use crate::music::sponsorblock::get_skips;
use crate::music::youtube_loudness::get_loudness;
use crate::PoiseContext;

pub enum Query {
    Search(String),
    Url(String),
}

pub async fn add_track(ctx: PoiseContext<'_>, query: Vec<Query>) -> Result<()> {
    let mutex = match get_lock(ctx).await {
        Err(_) => return Ok(()),
        Ok(m) => m,
    };
    let _lock = mutex.lock().await;

    let handler_lock = match ctx.get_voice().await {
        Ok(h) => h,
        Err(_) => {
            send_msg(
                ctx,
                SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
            )
            .await;
            return Ok(());
        }
    };

    let lazy = {
        // Workaround for https://github.com/serenity-rs/songbird/issues/97
        let handler = handler_lock.lock().await;
        !handler.queue().is_empty()
    };

    let mut tracks = futures::stream::iter(query.into_iter().enumerate().map(|(i, q)| {
        let lazy = lazy || (i != 0);
        create_track(ctx, q, lazy)
    }))
    .buffered(20);

    if let Ok(handler_lock) = ctx.join_voice().await {
        let mut handler = handler_lock.lock().await;
        handler.remove_all_global_events();

        while let Some(x) = tracks.next().await {
            if let Some((track, track_handle, sb_time)) = x {
                // Queue track
                handler.enqueue(track);

                // Make the next song in queue playable to reduce delay
                let queue = handler.queue().current_queue();
                if queue.len() > 1 {
                    let _ = queue[1].make_playable();
                }

                let update = match handler.queue().current_queue().len() {
                    1 => PlayUpdate::Play(queue.len(), sb_time),
                    _ => PlayUpdate::Add(queue.len()),
                };
                send_msg(
                    ctx,
                    SendMessage::Custom(format_update(&track_handle, update)),
                )
                .await;
            }
        }
    } else {
        send_msg(
            ctx,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

pub async fn remove_track(
    ctx: PoiseContext<'_>,
    start_idx: usize,
    end_idx: usize,
) -> Result<Vec<TrackHandle>, MusicError> {
    let mutex = match get_lock(ctx).await {
        Err(_) => return Err(MusicError::RemoveTrack),
        Ok(m) => m,
    };
    let _lock = mutex.lock().await;

    let handler_lock = ctx.get_voice().await.map_err(|_| MusicError::RemoveTrack)?;

    let handler = handler_lock.lock().await;
    let queue = handler.queue();

    // Remove requested track
    let mut removed_tracks = vec![];
    queue.modify_queue(|q| {
        for idx in (start_idx..=end_idx).rev() {
            if let Some(t) = q.remove(idx) {
                removed_tracks.push(t.handle());
                t.stop().unwrap_or(())
            };
        }
    });

    Ok(removed_tracks)
}

async fn create_track(
    ctx: PoiseContext<'_>,
    query: Query,
    lazy: bool,
) -> Option<(Track, TrackHandle, Option<Duration>)> {
    // Create source
    let source = match query {
        Query::Search(x) => Restartable::ytdl_search(x, lazy).await,
        Query::Url(x) => Restartable::ytdl(x.to_owned(), lazy).await,
    };
    let source = match source {
        Ok(s) => s,
        Err(_) => {
            send_msg(ctx, SendMessage::Error(MusicError::BadSource.as_str())).await;
            return None;
        }
    };

    let input: input::Input = source.into();

    // Get volume and skips
    let (volume, skips) = if let Some(url) = &input.metadata.source_url {
        tokio::join!(get_loudness(url), get_skips(url))
    } else {
        (1.0, vec![])
    };

    // Create track
    let (track, track_handle) = songbird::tracks::create_player(input);
    let _ = track_handle.set_volume(volume);

    // Set TrackSegmentSkipper if skips exist
    let sb_time = if let Some(segment) = skips.get(0).cloned() {
        let sb_time = skips.iter().map(|(a, b)| *b - *a).sum();
        track_handle
            .add_event(
                Event::Delayed(segment.0),
                TrackSegmentSkipper {
                    segments: skips,
                    idx: 0.into(),
                },
            )
            .unwrap();
        Some(sb_time)
    } else {
        None
    };

    // Set TrackEndNotifier
    track_handle
        .add_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                ctx: Arc::new(Mutex::new(ctx.discord().clone())),
                chan_id: ctx.channel_id(),
                guild_id: ctx.guild_id().unwrap(),
                http: ctx.discord().http.clone(),
            },
        )
        .expect("Error adding TrackEndNotifier");

    // Set TrackStartNotifier
    track_handle
        .add_event(
            Event::Track(TrackEvent::Play),
            TrackStartNotifier {
                ctx: Arc::new(Mutex::new(ctx.discord().clone())),
                chan_id: ctx.channel_id(),
                guild_id: ctx.guild_id().unwrap(),
                http: ctx.discord().http.clone(),
                sb_time,
            },
        )
        .expect("Error adding TrackStartNotifier");

    Some((track, track_handle, sb_time))
}

pub struct QueueMutexMap;

impl TypeMapKey for QueueMutexMap {
    type Value = HashMap<Option<GuildId>, Arc<Mutex<()>>>;
}

async fn get_lock(ctx: PoiseContext<'_>) -> Result<Arc<Mutex<()>>> {
    let data = ctx.discord().data.read().await;
    let map = data
        .get::<QueueMutexMap>()
        .ok_or(InsomniaError::QueueLock)?;
    let m = match map.get(&ctx.guild_id()) {
        Some(m) => m.clone(),
        None => {
            let m = Arc::new(Mutex::new(()));
            drop(data);
            let mut data = ctx.discord().data.write().await;
            let map = data
                .get_mut::<QueueMutexMap>()
                .ok_or(InsomniaError::QueueLock)?;
            map.insert(ctx.guild_id(), m.clone());
            m
        }
    };

    Ok(m)
}
