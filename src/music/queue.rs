use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::stream::StreamExt;
use poise::serenity_prelude as serenity;
use serenity::model::id::GuildId;
use serenity::*;
use songbird::input::{self, Restartable};
use songbird::tracks::{Track, TrackHandle};
use songbird::{Event, TrackEvent};

use super::error::MusicError;
use super::events::{TrackEndNotifier, TrackSegmentSkipper, TrackStartNotifier};
use super::message::{format_add_playlist, format_update, PlayUpdate};
use super::voice::{CanGetVoice, CanJoinVoice};
use crate::error::InsomniaError;
use crate::message::{edit_reply, send_msg, SendMessage, CANCEL_INTERACTION_ID};
use crate::music::sponsorblock::get_skips;
use crate::music::youtube_loudness::get_loudness;
use crate::PoiseContext;

pub enum Query {
    Search(String),
    Url(String),
}

pub async fn add_tracks(ctx: PoiseContext<'_>, queries: Vec<Query>) -> Result<()> {
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

    let num_queries = queries.len();
    let mut tracks = futures::stream::iter(queries.into_iter().enumerate().map(|(i, q)| {
        let lazy = lazy || (i != 0);
        create_track(ctx, q, lazy)
    }))
    .buffered(20);

    if let Ok(handler_lock) = ctx.join_voice().await {
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
            let discord = ctx.discord().clone();
            let channel_id = ctx.channel_id();
            tokio::spawn(async move {
                let button_interaction_fut = serenity::CollectComponentInteraction::new(&discord)
                    .channel_id(channel_id)
                    .filter(move |ci| ci.data.custom_id == CANCEL_INTERACTION_ID);
                tokio::select! {
                    _ = rx => (),
                    Some(ci) = button_interaction_fut => {
                        drop(tx_cancel);
                        ci.defer(discord.http()).await.unwrap();
                    },
                }
            });
        }

        while let Some(x) = tracks.next().await {
            if let Some((track, track_handle, sb_time)) = x {
                // Check if operation was cancelled
                if rx_cancel.try_recv().is_err() {
                    break;
                }

                let mut handler = handler_lock.lock().await;
                handler.remove_all_global_events();

                // Queue track
                handler.enqueue(track);

                // Make the next song in queue playable to reduce delay
                let queue = handler.queue().current_queue();
                if queue.len() > 1 {
                    let _ = queue[1].make_playable();
                }

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
                    edit_reply(
                        ctx,
                        reply_handle.clone(),
                        SendMessage::CustomCancelable(fmt()),
                    )
                    .await;
                } else {
                    let update = match handler.queue().current_queue().len() {
                        1 => PlayUpdate::Play(queue.len(), sb_time),
                        _ => PlayUpdate::Add(queue.len()),
                    };
                    if num_queries != 1 {
                        // If first of many queued tracks, send an initial reply
                        reply_handle =
                            Some(send_msg(ctx, SendMessage::CustomCancelable(fmt())).await);
                    }
                    if !(num_queries != 1 && matches!(update, PlayUpdate::Add(_))) {
                        send_msg(
                            ctx,
                            SendMessage::Custom(format_update(&track_handle, update)),
                        )
                        .await;
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
            edit_reply(ctx, reply_handle.clone(), SendMessage::Custom(fmt())).await;
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
