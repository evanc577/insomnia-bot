mod error;
mod events;
mod message;
pub mod queue;
mod sponsorblock;
pub mod voice;
mod youtube_loudness;
mod youtube_music;
mod youtube_playlist;

use crate::music::queue::add_track;
use crate::music::queue::Query;

use self::message::{format_update, PlayUpdate};
use self::voice::{CanGetVoice, CanJoinVoice};
use self::youtube_playlist::add_youtube_playlist;

use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};
use crate::message::{send_msg, SendMessage};
use crate::music::error::MusicError;

use if_chain::if_chain;
use serenity::http::Typing;
use serenity::{
    client::Context,
    framework::standard::{
        help_commands,
        macros::{command, group, help},
        Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::{channel::Message, id::UserId},
};
use songbird::tracks::{PlayMode, TrackHandle};
use std::collections::HashSet;

#[group]
#[commands(play, song, video, album, skip, stop, pause, list, remove)]
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
#[description = "Play a song via YouTube Music. If a URL is given, play the URL. If no argument is given, resume the paused track."]
#[usage = "[search_query | url]"]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Only allow if user is in a voice channel
    {
        match msg.get_voice(ctx).await {
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
    }

    if args.is_empty() {
        // If no arguments, resume current track
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
    } else {
        // Otherwise search/play the requested track
        if let Ok(url) = url::Url::parse(args.message()) {
            let _typing = Typing::start(ctx.http.clone(), msg.channel_id.0);
            if let Some(_) = add_youtube_playlist(ctx, msg, url.as_str()).await {
            } else {
                // If URL is given, play URL
                let _ = add_track(ctx, msg, vec![Query::URL(url.to_string())]).await;
            }
        } else {
            // Otherwise search YouTube Music
            let _ = song(ctx, msg, args).await;
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Play a song via YouTube Music."]
#[usage = "[search_query]"]
async fn song(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Only allow if user is in a voice channel
    {
        match msg.get_voice(ctx).await {
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
    }

    if let Some(url) = youtube_music::yt_music_song_search(args.message().to_owned()).await {
        let _typing = Typing::start(ctx.http.clone(), msg.channel_id.0);
        let _ = add_track(ctx, msg, vec![Query::URL(url.to_string())]).await;
    } else {
        send_msg(
            &ctx.http,
            msg.channel_id,
            SendMessage::Error(MusicError::BadSource.as_str()),
        )
        .await;
        return Ok(());
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Play an uploaded video's audio via YouTube."]
#[usage = "[search_query | url]"]
async fn video(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Only allow if user is in a voice channel
    {
        match msg.get_voice(ctx).await {
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
    }

    let _typing = Typing::start(ctx.http.clone(), msg.channel_id.0);

    if let Ok(url) = url::Url::parse(args.message()) {
        // If URL is given, play URL
        let _ = add_track(ctx, msg, vec![Query::URL(url.to_string())]).await;
    } else {
        // Otherwise search YouTube Music
        let _ = add_track(ctx, msg, vec![Query::Search(args.message().to_string())]).await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
#[description = "Play an album via YouTube Music."]
#[usage = "[search_query]"]
async fn album(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // Only allow if user is in a voice channel
    {
        match msg.get_voice(ctx).await {
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
    }

    let _typing = Typing::start(ctx.http.clone(), msg.channel_id.0);

    // Otherwise search YouTube Music
    if let Some(url) = youtube_music::yt_music_album_search(args.message().to_owned()).await {
        let _typing = Typing::start(ctx.http.clone(), msg.channel_id.0);
        if let Some(_) = add_youtube_playlist(ctx, msg, url.as_str()).await {
            return Ok(());
        }
    }

    send_msg(
        &ctx.http,
        msg.channel_id,
        SendMessage::Error(MusicError::BadSource.as_str()),
    )
    .await;

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
