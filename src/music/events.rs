use super::message::{format_update, PlayUpdate};
use crate::message::{send_msg, SendMessage};

use serenity::{async_trait, http::Http, model::prelude::*, prelude::*};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};

pub struct TrackSegmentSkipper {
    pub segments: Vec<(Duration, Duration)>,
    pub idx: AtomicUsize,
}

#[async_trait]
impl VoiceEventHandler for TrackSegmentSkipper {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let idx = self.idx.fetch_add(1, Ordering::SeqCst);
        if idx < self.segments.len() {
            if let EventContext::Track(&[(state, track)]) = ctx {
                let (seek_from, seek_to) = self.segments[idx];

                // Workaround for https://github.com/serenity-rs/songbird/issues/97
                let diff = seek_from.saturating_sub(state.position);
                if diff > Duration::from_secs(1) {
                    // Too early? Delay again
                    println!("{:?} too early!", diff);
                    self.idx.store(idx, Ordering::SeqCst);
                    return Some(Event::Delayed(diff));
                }

                // Skip to specified time
                track.seek_time(seek_to).unwrap();

                // If another skip exists, add an event
                if idx + 1 < self.segments.len() {
                    let next_segment = self.segments[idx + 1].0 - seek_to;
                    return Some(Event::Delayed(next_segment));
                }
            }
        }

        None
    }
}

pub struct TrackStartNotifier {
    pub ctx: Arc<Mutex<Context>>,
    pub chan_id: ChannelId,
    pub guild_id: GuildId,
    pub http: Arc<Http>,
    pub sb_time: Option<Duration>,
}

#[async_trait]
impl VoiceEventHandler for TrackStartNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let guild_ctx = self.ctx.lock().await;
        let manager = songbird::get(&guild_ctx)
            .await
            .expect("Songbird Voice client placed in at initialization.")
            .clone();
        if let EventContext::Track(&[(state, track)]) = ctx {
            let update = if state.position < Duration::from_secs(1) {
                let queue_len = if let Some(handler_lock) = manager.get(self.guild_id) {
                    let handler = handler_lock.lock().await;
                    handler.queue().len()
                } else {
                    1
                };
                PlayUpdate::Play(queue_len, self.sb_time)
            } else {
                PlayUpdate::Resume
            };
            send_msg(
                &self.http,
                self.chan_id,
                SendMessage::Custom(format_update(track, update)),
            )
            .await;
        }

        None
    }
}

/// Sets a global event which will leave the voice channel after while
pub struct TrackEndNotifier {
    pub ctx: Arc<Mutex<Context>>,
    pub chan_id: ChannelId,
    pub guild_id: GuildId,
    pub http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let ctx = self.ctx.lock().await;
        let manager = songbird::get(&ctx)
            .await
            .expect("Songbird Voice client placed in at initialization.")
            .clone();
        if let Some(handler_lock) = manager.get(self.guild_id) {
            let handler = handler_lock.lock().await;
            let queue = handler.queue();
            if !handler.queue().is_empty() {
                // Make the next song in queue playable to reduce delay
                if queue.len() > 1 {
                    let next_track = &queue.current_queue()[1];
                    let _ = next_track.make_playable();
                }
                return None;
            }
            drop(handler);

            send_msg(&self.http, self.chan_id, SendMessage::Normal("Queue ended")).await;

            set_leave_timer(handler_lock).await;
        }

        None
    }
}

async fn set_leave_timer(call: Arc<Mutex<songbird::Call>>) {
    let mut handle = call.lock().await;
    handle.add_global_event(
        Event::Delayed(Duration::from_secs(600)),
        ChannelIdleLeaver { call: call.clone() },
    );
}

struct ChannelIdleLeaver {
    call: Arc<Mutex<songbird::Call>>,
}

#[async_trait]
impl VoiceEventHandler for ChannelIdleLeaver {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let mut handler = self.call.lock().await;
        handler.leave().await.unwrap();
        None
    }
}
