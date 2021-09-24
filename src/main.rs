mod events;
mod msg;

use crate::events::{TrackEndNotifier, TrackStartNotifier};
use crate::msg::check_msg;

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
};
use songbird::{
    input::{self, restartable::Restartable},
    Event, SerenityInit, TrackEvent,
};
use std::{env, sync::Arc};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(play, skip, stop, list, remove)]
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

async fn join_or_get(
    ctx: &Context,
    msg: &Message,
    join: bool,
) -> Option<Arc<Mutex<songbird::Call>>> {
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
        .expect("Songbird Voice client placed in at initialization.")
        .clone();

    let handler_lock = if join {
        let handler_lock = manager.join(guild_id, connect_to).await.0;
        {
            let mut handler = handler_lock.lock().await;
            let _ = handler.deafen(true).await;
        }
        Some(handler_lock)
    } else {
        manager.get(guild_id)
    };

    handler_lock
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if args.is_empty() {
        return Ok(());
    }

    // Create source
    let arg_message = args.message();
    let source = if let Ok(url) = url::Url::parse(arg_message) {
        // Source is url, call ytdl directly
        Restartable::ytdl(url.as_str().to_owned(), true).await
    } else {
        // Otherwise search ytdl
        Restartable::ytdl_search(arg_message, true).await
    };
    let source = match source {
        Ok(s) => s,
        Err(e) => {
            println!("Error starting source: {:?}", e);
            check_msg(msg.reply(&ctx.http, "Error sourcing ffmpeg").await);
            return Ok(());
        }
    };

    let input: input::Input = source.into();
    let metadata = input.metadata.clone();
    let (track, track_handle) = songbird::tracks::create_player(input);
    let title = metadata.title.unwrap_or("Unknown".into());
    let artist = metadata.artist.unwrap_or("Unknown".into());

    if let Some(handler_lock) = join_or_get(ctx, msg, true).await {
        let mut handler = handler_lock.lock().await;

        // Set TrackEndNotifier
        handler.remove_all_global_events();
        track_handle
            .add_event(
                Event::Track(TrackEvent::End),
                TrackEndNotifier {
                    call: handler_lock.clone(),
                    chan_id: msg.channel_id,
                    http: ctx.http.clone(),
                },
            )
            .expect("Error adding TrackEndNotifier");

        // Set TrackStartNotifier
        track_handle
            .add_event(
                Event::Track(TrackEvent::Play),
                TrackStartNotifier {
                    artist: artist.clone(),
                    title: title.clone(),
                    chan_id: msg.channel_id,
                    http: ctx.http.clone(),
                },
            )
            .expect("Error adding TrackStartNotifier");

        handler.enqueue(track);

        let play_msg = match handler.queue().current_queue().len() {
            1 => format!("Playing {} - {}", artist, title),
            _ => format!(
                "Added {} - {}\nSongs in queue: {}",
                artist,
                title,
                handler.queue().len()
            ),
        };
        check_msg(msg.reply(&ctx.http, play_msg).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        queue.modify_queue(|q| {
            match q.pop_front() {
                Some(t) => t.stop().unwrap_or(()),
                None => (),
            };
        });
        if let Some(t) = queue.current() {
            let _ = t.play();
            let metadata = t.metadata().clone();
            check_msg(
                msg.reply(
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
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            let _ = track.stop();
        }
        let _ = queue.stop();

        check_msg(msg.reply(&ctx.http, "Queue cleared.").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn list(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse arguments
    let arg = match args.single::<String>() {
        Ok(a) => a,
        Err(_) => "1".to_owned(),
    };
    let start = match arg.to_lowercase().as_str() {
        "max" => None,
        s => Some(s.parse::<usize>().unwrap_or(0)),
    };

    // Build output string
    let (total, list) = if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue().current_queue();
        let queue_total = queue.len();

        let start = if let Some(i) = start {
            i
        } else {
            std::cmp::max(1, queue_total.saturating_sub(10))
        };

        let list = queue
            .iter()
            .zip(start..start + 10)
            .map(|(t, i)| {
                let artist = t
                    .metadata()
                    .artist
                    .as_ref()
                    .unwrap_or(&"Unknown".to_owned())
                    .to_owned();
                let title = t
                    .metadata()
                    .title
                    .as_ref()
                    .unwrap_or(&"Unknown".to_owned())
                    .to_owned();
                format!("{:>2}: {} - {}", i, artist, title)
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace("`", "");

        (queue_total, list)
    } else {
        return Ok(());
    };

    // Respond
    let out_msg = match total {
        0 => "Queue is empty".to_owned(),
        1 => format!("{} song in queue\n```{}```", 1, list),
        n => format!("{} songs in queue\n```{}```", n, list),
    };
    check_msg(msg.reply(&ctx.http, out_msg).await);

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn remove(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse arguments
    let arg = match args.single::<String>() {
        Ok(a) => a,
        Err(_) => {
            check_msg(msg.reply(&ctx.http, "Missing 1 argument").await);
            return Ok(());
        },
    };
    let idx = arg.to_lowercase().parse::<usize>();
    if let Ok(i) = idx {
        if i == 0 {
            check_msg(msg.reply(&ctx.http, "Invalid index").await);
            return Ok(());
        }
        let i = i - 1;

        if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
            let handler = handler_lock.lock().await;
            let queue = handler.queue();

            // Get info about the track to be removed
            let current_queue = queue.current_queue();
            let track = match current_queue.get(i) {
                Some(t) => t,
                None => {
                    check_msg(msg.reply(&ctx.http, "Invalid index").await);
                    return Ok(());
                }
            };
            let artist = track
                .metadata()
                .artist
                .as_ref()
                .unwrap_or(&"Unknown".to_owned())
                .to_owned();
            let title = track
                .metadata()
                .title
                .as_ref()
                .unwrap_or(&"Unknown".to_owned())
                .to_owned();

            // Remove requested track
            queue.modify_queue(|q| {
                match q.remove(i) {
                    Some(t) => t.stop().unwrap_or(()),
                    None => (),
                };
            });

            // Respond
            check_msg(
                msg.reply(&ctx.http, format!("Removed {} - {}", artist, title))
                    .await,
            );
        }
    } else {
        check_msg(msg.reply(&ctx.http, "Invalid argument").await);
    }

    Ok(())
}
