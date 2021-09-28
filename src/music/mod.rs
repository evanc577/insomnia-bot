mod events;
mod loudness;
mod message;
mod sponsorblock;
mod error;
mod voice;

use self::events::{TrackEndNotifier, TrackStartNotifier};
use self::loudness::get_loudness;
use self::message::{format_update, PlayUpdate};
use self::voice::{CanGetVoice, CanJoinVoice};
use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};
use crate::message::{send_msg, SendMessage};
use crate::music::error::MusicError;

use if_chain::if_chain;
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
    tracks::{PlayMode, TrackHandle},
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
async fn music_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let mut options = help_options.clone();
    options.embed_success_colour = *EMBED_COLOR;
    options.embed_error_colour = *EMBED_ERROR_COLOR;
    let _ = help_commands::with_embeds(context, msg, args, &options, groups, owners).await;
    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Play a track via YouTube. If no argument is given, will resume the paused track."]
#[usage = "[url | search_query]"]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Only allow if user is in a voice channel
    let handler_lock = match msg.get_voice(ctx).await {
        Ok(h) => h,
        Err(_) => {
            send_msg(
                &ctx.http,
                msg.channel_id,
                SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
            )
            .await;
            return Ok(());
        }
    };

    // If no arguments, resume current track
    if args.is_empty() {
        if_chain! {
            let handler = handler_lock.lock().await;
            let queue = handler.queue();
            if let Some(track) = queue.current();
            if let Ok(info) = track.get_info().await;
            if info.playing == PlayMode::Pause;
            then {
                let _ = track.play();
            } else {
                send_msg(
                    &ctx.http,
                    msg.channel_id,
                    SendMessage::Error(MusicError::NoPausedTrack.as_str()),
                )
                .await;
                return Ok(());
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
            send_msg(
                &ctx.http,
                msg.channel_id,
                SendMessage::Error(MusicError::BadSource.as_str()),
            )
            .await;
            return Ok(());
        }
    };

    let input: input::Input = source.into();
    let volume = if_chain! {
        if let Some(url) = &input.metadata.source_url;
        if let Ok(vol) = get_loudness(url).await;
        then {
            vol
        } else {
            1.0
        }
    };
    let (track, track_handle) = songbird::tracks::create_player(input);
    let _ = track_handle.set_volume(volume);

    // Set TrackEndNotifier
    track_handle
        .add_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                ctx: Arc::new(Mutex::new(ctx.clone())),
                chan_id: msg.channel_id,
                guild_id: msg.guild(&ctx.cache).await.unwrap().id,
                http: ctx.http.clone(),
            },
        )
        .expect("Error adding TrackEndNotifier");

    // Set TrackStartNotifier
    track_handle
        .add_event(
            Event::Track(TrackEvent::Play),
            TrackStartNotifier {
                ctx: Arc::new(Mutex::new(ctx.clone())),
                chan_id: msg.channel_id,
                guild_id: msg.guild(&ctx.cache).await.unwrap().id,
                http: ctx.http.clone(),
            },
        )
        .expect("Error adding TrackStartNotifier");

    if let Ok(handler_lock) = msg.join_voice(ctx).await {
        let mut handler = handler_lock.lock().await;
        handler.remove_all_global_events();

        // Queue track
        handler.enqueue(track);

        // Make the next song in queue playable to reduce delay
        let queue = handler.queue().current_queue();
        if queue.len() > 1 {
            let _ = queue[1].make_playable();
        }

        let update = match handler.queue().current_queue().len() {
            1 => PlayUpdate::Play(queue.len()),
            _ => PlayUpdate::Add(queue.len()),
        };
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Custom(format_update(&track_handle, update)),
        )
        .await;
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Pause the currently playing track."]
#[usage = ""]
async fn pause(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Ok(handler_lock) = msg.join_voice(ctx).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            if let Ok(info) = track.get_info().await {
                if info.playing != PlayMode::Play {
                    send_msg(
                        &ctx.http,
                        msg.channel_id,
                        SendMessage::Error(MusicError::NoPlayingTrack.as_str()),
                    )
                    .await;
                    return Ok(());
                }
            }
            let _ = track.pause();
            send_msg(
                &ctx.http,
                msg.channel_id,
                SendMessage::Custom(format_update(&track, PlayUpdate::Pause)),
            )
            .await;
        }
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Skip the currently playing track."]
#[usage = ""]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Ok(handler_lock) = msg.get_voice(ctx).await {
        let handler = handler_lock.lock().await;
        if let Some(track) = handler.queue().dequeue(0) {
            let _ = track.stop();
            send_msg(
                &ctx.http,
                msg.channel_id,
                SendMessage::Custom(format_update(&track, PlayUpdate::Skip)),
            )
            .await;
            if let Some(next) = handler.queue().current() {
                let _ = next.play();
            }
        }
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if let Ok(handler_lock) = msg.get_voice(ctx).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            let _ = track.stop();
        }
        let _ = queue.stop();
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
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

    let (total, list) = if let Ok(handler_lock) = msg.get_voice(ctx).await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue().current_queue();
        let queue_total = queue.len();

        // Parse arguments
        let arg = match args.single::<String>() {
            Ok(a) => a,
            Err(_) => "1".to_owned(),
        };
        let start = match arg.to_lowercase().as_str() {
            "max" => None,
            s => Some(s.parse::<usize>().unwrap_or(0)),
        };

        // Compute start index
        let start = if let Some(i) = start {
            i
        } else {
            std::cmp::max(1, queue_total.saturating_sub(NUM_TRACKS))
        };

        // Build output string
        let list = queue
            .iter()
            .zip(start..start + NUM_TRACKS)
            .map(|(t, i)| format!("{:>2}: {}", i, format_track(t)))
            .collect::<Vec<_>>()
            .join("\n");

        (queue_total, list)
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
        return Ok(());
    };

    // Respond
    let out_msg = match total {
        0 => "Queue is empty".to_owned(),
        1 => format!("{} track in queue\n```{}```", 1, list),
        n => format!("{} tracks in queue\n```{}```", n, list),
    };
    send_msg(&ctx.http, msg.channel_id, SendMessage::Normal(&out_msg)).await;

    Ok(())
}

fn format_track(track: &TrackHandle) -> String {
    let title = track
        .metadata()
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());

    title.replace("`", "")
}

#[command]
#[only_in(guilds)]
#[aliases("rm")]
#[description = "Remove a track from the queue."]
#[usage = "track_number"]
async fn remove(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if let Ok(handler_lock) = msg.get_voice(ctx).await {
        // Parse arguments
        let arg = match args.single::<String>() {
            Ok(a) => a,
            Err(_) => {
                send_msg(
                    &ctx.http,
                    msg.channel_id,
                    SendMessage::Error(MusicError::BadArgument.as_str()),
                )
                .await;
                return Ok(());
            }
        };
        let idx = arg.to_lowercase().parse::<usize>();
        if let Ok(i) = idx {
            if i == 0 {
                send_msg(
                    &ctx.http,
                    msg.channel_id,
                    SendMessage::Error(MusicError::BadIndex.as_str()),
                )
                .await;
                return Ok(());
            }
            let i = i - 1;

            let handler = handler_lock.lock().await;
            let queue = handler.queue();

            // Get info about the track to be removed
            let current_queue = queue.current_queue();
            let track = match current_queue.get(i) {
                Some(t) => t,
                None => {
                    send_msg(
                        &ctx.http,
                        msg.channel_id,
                        SendMessage::Error(MusicError::BadIndex.as_str()),
                    )
                    .await;
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
            send_msg(
                &ctx.http,
                msg.channel_id,
                SendMessage::Custom(format_update(track, PlayUpdate::Remove)),
            )
            .await;
        } else {
            send_msg(
                &ctx.http,
                msg.channel_id,
                SendMessage::Error(MusicError::BadArgument.as_str()),
            )
            .await;
        }
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}
