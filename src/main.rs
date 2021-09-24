//! Example demonstrating how to make use of individual track audio events,
//! and how to use the `TrackQueue` system.
//!
//! Requires the "cache", "standard_framework", and "voice" features be enabled in your
//! Cargo.toml, like so:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["cache", "framework", "standard_framework", "voice"]
//! ```
use std::{
    collections::VecDeque,
    env,
    sync::Arc,
    time::Duration,
};

use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::{
        standard::{
            macros::{command, group},
            Args, CommandResult,
        },
        StandardFramework,
    },
    model::{channel::Message, gateway::Ready},
    prelude::*,
    Result as SerenityResult,
};

use songbird::{
    input::{self, restartable::Restartable},
    Event, EventContext, EventHandler as VoiceEventHandler, SerenityInit, TrackEvent,
};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(leave, play, skip, stop)]
struct General;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~"))
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}

async fn join(ctx: &Context, msg: &Message) -> Option<Arc<Mutex<songbird::Call>>> {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return None;
        }
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = if let Some(call) = manager.get(guild_id) {
        call
    } else {
        let handler_lock = manager.join(guild_id, connect_to).await.0;
        {
            let mut handler = handler_lock.lock().await;
            let _ = handler.deafen(true).await;
        }
        handler_lock
    };

    Some(handler_lock)
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        }
    };

    let mut handler = handler_lock.lock().await;

    if handler.is_mute() {
        check_msg(msg.channel_id.say(&ctx.http, "Already muted").await);
    } else {
        if let Err(e) = handler.mute(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Now muted").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await,
            );

            return Ok(());
        }
    };

    if !url.starts_with("http") {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Must provide a valid URL")
                .await,
        );

        return Ok(());
    }

    // Here, we use lazy restartable sources to make sure that we don't pay
    // for decoding, playback on tracks which aren't actually live yet.
    let source = match Restartable::ytdl(url, true).await {
        Ok(source) => source,
        Err(why) => {
            println!("Err starting source: {:?}", why);
            check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);
            return Ok(());
        }
    };

    let input: input::Input = source.into();
    let metadata = input.metadata.clone();
    let (track, track_handle) = songbird::tracks::create_player(input);

    if let Some(handler_lock) = join(ctx, msg).await {
        let mut handler = handler_lock.lock().await;

        handler.remove_all_global_events();
        track_handle
            .add_event(
                Event::Track(TrackEvent::End),
                TrackEndNotifier {
                    call: handler_lock.clone(),
                },
            )
            .expect("Error adding TrackEndNotifier");

        handler.enqueue(track);

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "Added {} - {}\nSongs in queue: {}",
                        metadata.artist.unwrap_or("Unknown".into()),
                        metadata.title.unwrap_or("Unknown".into()),
                        handler.queue().len(),
                    ),
                )
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(handler_lock) = join(ctx, msg).await {
        let handler = handler_lock.lock().await;
        let remove_first = |q: &mut VecDeque<songbird::tracks::Queued>| {
            match q.pop_front() {
                Some(t) => t.stop().unwrap_or(()),
                None => (),
            };
        };
        let queue = handler.queue();
        queue.modify_queue(remove_first);
        if let Some(t) = queue.current() {
            let _ = t.play();
            let metadata = t.metadata().clone();
            check_msg(
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!(
                            "Now playing {} - {}\nSongs in queue: {}",
                            metadata.artist.unwrap_or("Unknown".into()),
                            metadata.title.unwrap_or("Unknown".into()),
                            queue.len(),
                        ),
                    )
                    .await,
            );
        } else {
            check_msg(msg.channel_id.say(&ctx.http, "Queue ended").await);
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(handler_lock) = join(ctx, msg).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            let _ = track.stop();
        }
        let _ = queue.stop();

        check_msg(msg.channel_id.say(&ctx.http, "Queue cleared.").await);
    }

    Ok(())
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

struct TrackEndNotifier {
    call: Arc<Mutex<songbird::Call>>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        if !self.call.lock().await.queue().is_empty() {
            return None;
        }
        set_leave_timer(self.call.clone()).await;

        None
    }
}

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
