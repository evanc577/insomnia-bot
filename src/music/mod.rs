mod events;
mod sponsorblock;

use crate::msg::{check_msg, format_track};
use events::{TrackEndNotifier, TrackStartNotifier};
use sponsorblock::get_skips;

use serenity::{
    client::Context,
    framework::standard::{
        help_commands,
        macros::{command, group, help},
        Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::{channel::Message, id::UserId},
    prelude::*,
};
use songbird::{
    input::{self, restartable::Restartable},
    tracks::PlayMode,
    Event, TrackEvent,
};
use std::{collections::HashSet, sync::Arc};

#[group]
#[commands(play, skip, stop, pause, list, remove)]
pub struct Music;

#[help]
#[individual_command_tip = ""]
#[strikethrough_commands_tip_in_guild = ""]
#[max_levenshtein_distance(1)]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
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
#[description = "Play a track via YouTube. If no argument is given, will resume the paused track."]
#[usage = "[url | search_query]"]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // If no arguments, resume current track
    if args.is_empty() {
        if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
            let handler = handler_lock.lock().await;
            let queue = handler.queue();
            if let Some(track) = queue.current() {
                if let Ok(info) = track.get_info().await {
                    if info.playing != PlayMode::Pause {
                        check_msg(msg.reply(&ctx.http, "No paused track").await);
                        return Ok(());
                    }
                }
                let _ = track.play();
                check_msg(
                    msg.reply(&ctx.http, format!("Resuming {}", format_track(&track, true)))
                        .await,
                );
            }
        }

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
        Err(_) => {
            check_msg(msg.reply(&ctx.http, "Error loading source").await);
            return Ok(());
        }
    };

    let input: input::Input = source.into();
    let (track, track_handle) = songbird::tracks::create_player(input);
    if let Some(u) = &track_handle.metadata().source_url {
        // get_skips(&u).await;
    }

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
                    name: format_track(&track_handle, true),
                    chan_id: msg.channel_id,
                    http: ctx.http.clone(),
                },
            )
            .expect("Error adding TrackStartNotifier");

        // Queue track
        handler.enqueue(track);

        // Make the next song in queue playable to reduce delay
        let queue = handler.queue().current_queue();
        if queue.len() > 1 {
            let _ = queue[1].make_playable();
        }

        let play_msg = match handler.queue().current_queue().len() {
            1 => format!("Playing {}", format_track(&track_handle, true)),
            _ => format!(
                "Added {}\nTracks in queue: {}",
                format_track(&track_handle, true),
                queue.len()
            ),
        };
        check_msg(msg.reply(&ctx.http, play_msg).await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Pause the currently playing track."]
#[usage = ""]
async fn pause(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            if let Ok(info) = track.get_info().await {
                if info.playing != PlayMode::Play {
                    check_msg(msg.reply(&ctx.http, "No playing track").await);
                    return Ok(());
                }
            }
            let _ = track.pause();
            check_msg(
                msg.reply(&ctx.http, format!("Paused {}", format_track(&track, true)))
                    .await,
            );
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Skip the currently playing track."]
#[usage = ""]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Some(handler_lock) = join_or_get(ctx, msg, false).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            let _ = track.stop();
            check_msg(
                msg.reply(&ctx.http, format!("Skipped {}", format_track(&track, true)))
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
        check_msg(msg.reply(&ctx.http, "Playback stopped").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[aliases("ls", "queue")]
#[description = "List all tracks in queue."]
#[usage = "[track_number]"]
async fn list(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    const NUM_TRACKS: usize = 25;
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
            std::cmp::max(1, queue_total.saturating_sub(NUM_TRACKS))
        };

        let list = queue
            .iter()
            .zip(start..start + NUM_TRACKS)
            .map(|(t, i)| {
                format!("{:>2}: {}", i, format_track(&t, false))
            })
            .collect::<Vec<_>>()
            .join("\n");

        (queue_total, list)
    } else {
        return Ok(());
    };

    // Respond
    let out_msg = match total {
        0 => "Queue is empty".to_owned(),
        1 => format!("{} track in queue\n```{}```", 1, list),
        n => format!("{} tracks in queue\n```{}```", n, list),
    };
    check_msg(msg.reply(&ctx.http, out_msg).await);

    Ok(())
}

#[command]
#[only_in(guilds)]
#[aliases("rm")]
#[description = "Remove a track from the queue."]
#[usage = "track_number"]
async fn remove(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse arguments
    let arg = match args.single::<String>() {
        Ok(a) => a,
        Err(_) => {
            check_msg(msg.reply(&ctx.http, "Missing 1 argument").await);
            return Ok(());
        }
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

            // Remove requested track
            queue.modify_queue(|q| {
                if let Some(t) = q.remove(i) {
                    t.stop().unwrap_or(())
                };
            });

            // Respond
            check_msg(
                msg.reply(&ctx.http, format!("Removed {}", format_track(&track, true)))
                    .await,
            );
        }
    } else {
        check_msg(msg.reply(&ctx.http, "Invalid argument").await);
    }

    Ok(())
}
