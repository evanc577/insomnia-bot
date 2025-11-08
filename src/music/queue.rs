use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

use anyhow::Result;
use futures::stream::StreamExt;
use futures::Stream;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::prelude::TypeMapKey;
use serenity::model::id::GuildId;
use serenity::*;
use songbird::events::EventStore;
use songbird::input::{self, Compose};
use songbird::tracks::{Track, TrackHandle};
use songbird::{Event, TrackEvent};

use super::error::{InternalError, MusicError};
use super::events::{TrackEndNotifier, TrackSegmentSkipper, TrackStartNotifier};
use super::message::{format_add_playlist, PlayUpdate};
use super::voice::{CanGetVoice, CanJoinVoice};
use super::youtube::loudness::get_loudness;
use super::youtube::sponsorblock::{get_skips, SBDuration};
use crate::message::{CustomSendMessage, SendableMessage, CANCEL_INTERACTION_ID};
use crate::music::music_data::MusicData;
use crate::PoiseContext;

pub enum Query {
    Search(String),
    Url(String),
}

/// Add the given tracks to the queue
pub async fn add_tracks(
    ctx: PoiseContext<'_>,
    queries: impl Stream<Item = Query>,
    num_queries: usize,
) -> Result<(), MusicError> {
    let mutex = get_lock(ctx).await?;
    let _lock = mutex.lock().await;

    let handler_lock = ctx.get_voice().await?;

    let lazy = {
        // Workaround for https://github.com/serenity-rs/songbird/issues/97
        let handler = handler_lock.lock().await;
        !handler.queue().is_empty()
    };

    tokio::pin!(queries);
    let mut tracks = queries
        .enumerate()
        .map(|(i, q)| {
            let lazy = lazy || (i != 0);
            create_track(ctx, q, lazy)
        })
        .buffered(20);

    // If adding more than 1 track, keep track of previously queued tracks for reply
    let mut reply_handle: Option<poise::ReplyHandle> = None;
    const MAX_NUM_DISPLAYED_TRACKS: usize = 10;
    let mut pushed_tracks = VecDeque::with_capacity(MAX_NUM_DISPLAYED_TRACKS);
    let mut num_queued_tracks = 0;

    // Spawn a new task to check if:
    // _tx is dropped when all tracks are added
    let (_tx, rx) = futures::channel::oneshot::channel::<()>();
    // OR tx_cancel is dropped when cancel button is pressed
    let (tx_cancel, mut rx_cancel) = futures::channel::oneshot::channel::<()>();
    {
        let serenity_ctx = ctx.serenity_context().clone();
        let channel_id = ctx.channel_id();
        tokio::spawn(async move {
            let button_interaction_fut = serenity::CollectComponentInteraction::new(&serenity_ctx)
                .channel_id(channel_id)
                .filter(move |ci| ci.data.custom_id == CANCEL_INTERACTION_ID);
            tokio::select! {
                _ = rx => (),
                Some(ci) = button_interaction_fut => {
                    drop(tx_cancel);
                    ci.defer(serenity_ctx.http()).await.unwrap();
                },
            }
        });
    }

    while let Some(x) = tracks.next().await {
        match x {
            Err(e) => {
                if num_queries == 1 {
                    return Err(e);
                }
            }
            Ok((track, track_handle)) => {
                // Check if operation was cancelled
                if rx_cancel.try_recv().is_err() {
                    break;
                }

                let handler_lock = ctx.join_voice().await?;
                let mut handler = handler_lock.lock().await;
                handler.remove_all_global_events();

                // Queue track
                handler.enqueue(track);

                // Make the next song in queue playable to reduce delay
                let queue = handler.queue().current_queue();

                // Updated tracks to send in reply
                if pushed_tracks.len() >= MAX_NUM_DISPLAYED_TRACKS {
                    pushed_tracks.pop_front();
                }
                pushed_tracks.push_back(track_handle.clone());
                num_queued_tracks += 1;

                // Send/edit reply
                let fmt = || {
                    format_add_playlist(
                        pushed_tracks.clone().into_iter(),
                        num_queued_tracks,
                        num_queries,
                        false,
                    )
                };
                if let Some(ref reply_handle) = reply_handle {
                    // If a previous reply has been sent, edit the reply
                    CustomSendMessage::Cancelable(fmt())
                        .edit_reply(ctx, reply_handle.clone())
                        .await;
                } else {
                    let update = match handler.queue().current_queue().len() {
                        1 => PlayUpdate::Play(track_handle.clone(), queue.len()),
                        _ => PlayUpdate::Add(track_handle.clone(), queue.len()),
                    };
                    if num_queries != 1 {
                        // If first of many queued tracks, send an initial reply
                        reply_handle =
                            Some(CustomSendMessage::Cancelable(fmt()).send_msg(ctx).await);
                    }
                    if !(num_queries != 1 && matches!(update, PlayUpdate::Add(_, _))) {
                        CustomSendMessage::Custom(update.format().await)
                            .send_msg(ctx)
                            .await;
                    }
                }
            }
        }
    }

    // Finish editing reply once all tracks are queued
    if let Some(ref reply_handle) = reply_handle {
        let fmt = || {
            format_add_playlist(
                pushed_tracks.into_iter(),
                num_queued_tracks,
                num_queries,
                true,
            )
        };
        CustomSendMessage::Custom(fmt())
            .edit_reply(ctx, reply_handle.clone())
            .await;
    }

    if num_queries == num_queued_tracks {
        Ok(())
    } else {
        Err(MusicError::AddTracks {
            failed: num_queries - num_queued_tracks,
            total: num_queries,
        })
    }
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
) -> Result<(Track, TrackHandle), MusicError> {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| reqwest::Client::new());

    // Create source
    let ytdl = match query {
        Query::Search(query) => input::YoutubeDl::new_search(*CLIENT, query),
        Query::Url(url) => input::YoutubeDl::new(*CLIENT, url),
    };

    // Get volume and skips
    let (volume, skips) = if let Some(url) = &ytdl
        .aux_metadata()
        .await
        .map_err(|e| MusicError::BadSource(e.to_string()))?
        .source_url
    {
        tokio::join!(get_loudness(url), get_skips(url))
    } else {
        (1.0, vec![])
    };

    // Create track
    let input: input::Input = ytdl.into();
    let title = input
        .aux_metadata()
        .await
        .ok()
        .map(|m| m.title)
        .flatten()
        .unwrap_or_else(|| "Unknown".to_owned());
    let track = Track::new_with_data(input, Arc::new(MusicData { title }));

    // Set volume
    track.volume(volume);

    let mut evt_store = EventStore::new_local();

    // Set TrackSegmentSkipper if skips exist
    let sb_time = if let Some(segment) = skips.first().cloned() {
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
    track_handle
        .typemap()
        .write()
        .await
        .insert::<SBDuration>(sb_time);

    // Set TrackEndNotifier
    track_handle
        .add_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                ctx: Arc::new(Mutex::new(ctx.serenity_context().clone())),
                chan_id: ctx.channel_id(),
                guild_id: ctx.guild_id().unwrap(),
                http: ctx.serenity_context().http.clone(),
            },
        )
        .expect("Error adding TrackEndNotifier");

    // Set TrackStartNotifier
    track_handle
        .add_event(
            Event::Track(TrackEvent::Play),
            TrackStartNotifier {
                ctx: Arc::new(Mutex::new(ctx.serenity_context().clone())),
                chan_id: ctx.channel_id(),
                guild_id: ctx.guild_id().unwrap(),
                http: ctx.serenity_context().http.clone(),
            },
        )
        .expect("Error adding TrackStartNotifier");

    Ok((track, track_handle))
}

pub struct QueueMutexMap;

impl TypeMapKey for QueueMutexMap {
    type Value = HashMap<Option<GuildId>, Arc<Mutex<()>>>;
}

async fn get_lock(ctx: PoiseContext<'_>) -> Result<Arc<Mutex<()>>, MusicError> {
    let data = ctx.serenity_context().data.read().await;
    let map = data
        .get::<QueueMutexMap>()
        .ok_or_else(|| MusicError::Internal(InternalError::QueueLock.into()))?;
    let m = match map.get(&ctx.guild_id()) {
        Some(m) => m.clone(),
        None => {
            let m = Arc::new(Mutex::new(()));
            drop(data);
            let mut data = ctx.serenity_context().data.write().await;
            let map = data
                .get_mut::<QueueMutexMap>()
                .ok_or_else(|| MusicError::Internal(InternalError::QueueLock.into()))?;
            map.insert(ctx.guild_id(), m.clone());
            m
        }
    };

    Ok(m)
}
