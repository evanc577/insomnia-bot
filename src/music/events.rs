use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude as serenity;
use serenity::http::Http;
use serenity::{async_trait, *};
use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};

use super::message::PlayUpdate;
use crate::message::{CustomSendMessage, SendableMessage};

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
                PlayUpdate::Play(track.clone(), queue_len)
            } else {
                PlayUpdate::Resume(track.clone())
            };
            CustomSendMessage::Custom(update.format().await)
                .send_msg_http(&self.http, self.chan_id)
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

            // Need to figure out how to prevent this message spamming chat when stopping queue
            // with multiple tracks
            // send_msg(&self.http, self.chan_id, SendMessage::Normal("Queue ended")).await;

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
        if let Some(track) = handler.queue().current() {
            let _ = track.stop();
        }
        handler.queue().stop();
        handler.leave().await.unwrap();
        handler.remove_all_global_events();
        None
    }
}

/// If bot is in a voice channel when the last other user leaves, set an idle timeout
pub async fn handle_voice_state_event(
    ctx: &serenity::Context,
    voice_state: &serenity::model::voice::VoiceState,
) {
    if let Some(guild_id) = voice_state.guild_id {
        let songbird = songbird::serenity::get(ctx).await.unwrap();
        let handler_lock = if let Some(h) = songbird.get(guild_id) {
            h
        } else {
            return;
        };

        let cache = ctx.cache.clone();
        let guild = cache
            .guild(guild_id)
            .unwrap()
            .channels(ctx.http())
            .await
            .unwrap();
        for (_, channel) in guild {
            if channel.kind != ChannelType::Voice {
                continue;
            }

            // Find the voice channel bot is in
            let mut bot_in_channel = false;
            let bot_user_id = cache.clone().current_user_id();
            let members = channel.members(cache.clone()).await.unwrap();
            for member in &members {
                if member.user.id == bot_user_id {
                    bot_in_channel = true;
                    break;
                }
            }

            if bot_in_channel {
                if members.len() == 1 {
                    // Only bot is in channel, add idle timeout
                    set_leave_timer(handler_lock).await;
                } else {
                    // Others in channel as well, remove idle timeout if queue is not empty
                    let mut handler = handler_lock.lock().await;
                    if !handler.queue().is_empty() {
                        handler.remove_all_global_events();
                    }
                }
                return;
            }
        }
    }
}
