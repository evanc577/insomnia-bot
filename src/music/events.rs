use crate::music::check_msg;

use serenity::{async_trait, http::Http, model::prelude::*, prelude::*};
use std::{sync::Arc, time::Duration};

use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};

pub struct TrackStartNotifier {
    pub name: String,
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for TrackStartNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        check_msg(
            self.chan_id
                .say(&self.http, &format!("Playing {}", self.name))
                .await,
        );
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
        {
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
                check_msg(self.chan_id.say(&self.http, "Queue finished").await);
                drop(handler);

                set_leave_timer(handler_lock).await;
            }
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
