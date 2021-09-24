use crate::check_msg;

use serenity::{async_trait, http::Http, model::prelude::ChannelId, prelude::*};
use std::{sync::Arc, time::Duration};

use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};

pub struct TrackStartNotifier {
    pub artist: String,
    pub title: String,
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for TrackStartNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        check_msg(
            self.chan_id
                .say(
                    &self.http,
                    &format!("Playing {} - {}", self.artist, self.title),
                )
                .await,
        );
        None
    }
}

/// Sets a global event which will leave the voice channel after while
pub struct TrackEndNotifier {
    pub call: Arc<Mutex<songbird::Call>>,
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        {
            let handler = self.call.lock().await;
            if !handler.queue().is_empty() {
                return None;
            }
            check_msg(
                self.chan_id
                    .say(&self.http, &format!("Queue finished"))
                    .await,
            );
        }
        set_leave_timer(self.call.clone()).await;

        None
    }
}

async fn set_leave_timer(call: Arc<Mutex<songbird::Call>>) {
    let mut handle = call.lock().await;
    handle.add_global_event(
        Event::Delayed(Duration::from_secs(5)),
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
