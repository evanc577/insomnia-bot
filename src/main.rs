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
use std::{collections::VecDeque, env, sync::Arc};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(play, skip, stop)]
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
        .expect("Songbird Voice client placed in at initialization.")
        .clone();

    let handler_lock = manager.join(guild_id, connect_to).await.0;
    {
        let mut handler = handler_lock.lock().await;
        let _ = handler.deafen(true).await;
    }

    Some(handler_lock)
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
    let title = metadata.title.unwrap_or("Unknown".into());

    if let Some(handler_lock) = join(ctx, msg).await {
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
                    title: title.clone(),
                    chan_id: msg.channel_id,
                    http: ctx.http.clone(),
                },
            )
            .expect("Error adding TrackStartNotifier");

        handler.enqueue(track);

        let play_msg = match handler.queue().current_queue().len() {
            1 => format!("Playing {}", title),
            _ => format!("Added {}\nSongs in queue: {}", title, handler.queue().len()),
        };
        check_msg(msg.channel_id.say(&ctx.http, play_msg).await);
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
