mod error;
mod events;
mod message;
pub mod queue;
mod sponsorblock;
pub mod voice;
mod youtube_loudness;
mod youtube_music;
mod youtube_playlist;

use if_chain::if_chain;
use songbird::tracks::{PlayMode, TrackHandle};

use self::message::{format_update, format_update_title_only, PlayUpdate};
use self::queue::remove_track;
use self::voice::{CanGetVoice, CanJoinVoice};
use self::youtube_playlist::add_youtube_playlist;
use crate::message::{send_msg, SendMessage};
use crate::music::error::MusicError;
use crate::music::queue::{add_track, Query};
use crate::{Error, PoiseContext};

/// Play a song via YouTube Music or URL, if no argument is given, resume the paused track
#[poise::command(slash_command, prefix_command, guild_only, broadcast_typing)]
pub async fn play(
    ctx: PoiseContext<'_>,
    #[rest]
    #[description = "Song title or URL"]
    #[rename = "song_or_url"]
    arg: Option<String>,
) -> Result<(), Error> {
    if !ctx.in_voice_and_send_msg().await {
        return Ok(());
    }

    if let Some(arg) = arg {
        ctx.defer_or_broadcast().await?;
        if let Ok(url) = url::Url::parse(&arg) {
            if add_youtube_playlist(ctx, url.as_str()).await.is_some() {
            } else {
                // If URL is given, play URL
                let _ = add_track(ctx, vec![Query::Url(url.to_string())]).await;
            }
        } else {
            // Otherwise search YouTube Music
            add_song(&ctx, arg).await?;
        }
    } else {
        // If no arguments, resume current track
        let handler_lock = match ctx.get_voice().await {
            Ok(h) => h,
            Err(_) => {
                send_msg(
                    ctx,
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
                    ctx,
                    SendMessage::Error(MusicError::NoPausedTrack.as_str()),
                )
                .await;
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Play a song via YouTube Music
#[poise::command(slash_command, prefix_command, guild_only, broadcast_typing)]
pub async fn song(
    ctx: PoiseContext<'_>,
    #[rest]
    #[description = "Song title"]
    #[rename = "song"]
    arg: String,
) -> Result<(), Error> {
    if !ctx.in_voice_and_send_msg().await {
        return Ok(());
    }

    add_song(&ctx, arg).await
}

async fn add_song(ctx: &PoiseContext<'_>, song: String) -> Result<(), Error> {
    ctx.defer_or_broadcast().await?;
    if let Some(url) = youtube_music::yt_music_song_search(song).await {
        let _ = add_track(*ctx, vec![Query::Url(url.to_string())]).await;
    } else {
        send_msg(*ctx, SendMessage::Error(MusicError::BadSource.as_str())).await;
        return Ok(());
    }

    Ok(())
}

/// Play a YouTube video
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    broadcast_typing,
    aliases("yt", "youtube")
)]
pub async fn video(
    ctx: PoiseContext<'_>,
    #[rest]
    #[description = "Search query or URL"]
    #[rename = "query_or_url"]
    arg: String,
) -> Result<(), Error> {
    if !ctx.in_voice_and_send_msg().await {
        return Ok(());
    }

    ctx.defer_or_broadcast().await?;
    if let Ok(url) = url::Url::parse(&arg) {
        // If URL is given, play URL
        let _ = add_track(ctx, vec![Query::Url(url.to_string())]).await;
    } else {
        // Otherwise search YouTube Music
        let _ = add_track(ctx, vec![Query::Search(arg)]).await;
    }

    Ok(())
}

/// Play an album via YouTube Music
#[poise::command(slash_command, prefix_command, guild_only, broadcast_typing)]
pub async fn album(
    ctx: PoiseContext<'_>,
    #[rest]
    #[description = "Album title"]
    #[rename = "album"]
    arg: String,
) -> Result<(), Error> {
    if !ctx.in_voice_and_send_msg().await {
        return Ok(());
    }

    // Otherwise search YouTube Music
    ctx.defer_or_broadcast().await?;
    if let Some(url) = youtube_music::yt_music_album_search(arg).await {
        if add_youtube_playlist(ctx, url.as_str()).await.is_some() {
            return Ok(());
        }
    }

    send_msg(ctx, SendMessage::Error(MusicError::BadSource.as_str())).await;

    Ok(())
}

/// Pause the currently playing track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn pause(ctx: PoiseContext<'_>) -> Result<(), Error> {
    if let Ok(handler_lock) = ctx.join_voice().await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if_chain! {
            if let Some(track) = queue.current();
            if let Ok(info) = track.get_info().await;
            if info.playing == PlayMode::Play;
            then {
                track.pause()?;
                send_msg(
                    ctx,
                    SendMessage::Custom(format_update(&track, PlayUpdate::Pause)),
                )
                .await;
            } else {
                send_msg(ctx, SendMessage::Error(MusicError::NoPlayingTrack.as_str())).await;
            }
        }
    } else {
        send_msg(
            ctx,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

/// Skip the currently playing track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn skip(ctx: PoiseContext<'_>) -> Result<(), Error> {
    if let Ok(handler_lock) = ctx.get_voice().await {
        let handler = handler_lock.lock().await;
        if let Some(track) = handler.queue().dequeue(0) {
            let _ = track.stop();
            send_msg(
                ctx,
                SendMessage::Custom(format_update(&track, PlayUpdate::Skip)),
            )
            .await;
            if let Some(next) = handler.queue().current() {
                let _ = next.play();
            }
        }
    } else {
        send_msg(
            ctx,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

/// Stop playing and clear queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn stop(ctx: PoiseContext<'_>) -> Result<(), Error> {
    if let Ok(handler_lock) = ctx.get_voice().await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Some(track) = queue.current() {
            let _ = track.stop();
        }
        let _ = queue.stop();
        send_msg(
            ctx,
            SendMessage::Custom(format_update_title_only(PlayUpdate::Stop)),
        )
        .await;
    } else {
        send_msg(
            ctx,
            SendMessage::Error(MusicError::NotInVoiceChannel.as_str()),
        )
        .await;
    }

    Ok(())
}

/// List tracks in queue
#[poise::command(slash_command, prefix_command, guild_only, aliases("ls", "queue"))]
pub async fn list(
    ctx: PoiseContext<'_>,
    #[rename = "index"]
    #[description = "List starting from this index in queue"]
    arg: Option<String>,
) -> Result<(), Error> {
    const NUM_TRACKS: usize = 25;

    let (total, list) = if let Ok(handler_lock) = ctx.get_voice().await {
        let handler = handler_lock.lock().await;
        let queue = handler.queue().current_queue();
        let queue_total = queue.len();

        // Parse arguments
        let arg = match arg {
            Some(a) => a,
            None => "1".to_owned(),
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
            ctx,
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
    send_msg(ctx, SendMessage::Normal(&out_msg)).await;

    Ok(())
}

fn format_track(track: &TrackHandle) -> String {
    let title = track
        .metadata()
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());

    title.replace('`', "")
}

/// Remove tracks from queue
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    broadcast_typing,
    aliases("rm")
)]
pub async fn remove(
    ctx: PoiseContext<'_>,
    #[description = "Track number to remove"] track: usize,
    #[description = "Remove tracks between indices (inclusive)"] track_end: Option<usize>,
) -> Result<(), Error> {
    if !ctx.in_voice_and_send_msg().await {
        return Ok(());
    }

    // Parse arguments
    let start_idx = track - 1;
    let end_idx = match track_end {
        Some(i) => i - 1,
        None => start_idx,
    };

    // Remove tracks
    match remove_track(ctx, start_idx, end_idx).await {
        Ok(removed) => {
            if removed.len() == 1 {
                send_msg(
                    ctx,
                    SendMessage::Custom(format_update(&removed[0], PlayUpdate::Remove)),
                )
                .await;
            } else {
                send_msg(
                    ctx,
                    SendMessage::Normal(&format!("Removed {} tracks", removed.len())),
                )
                .await;
            }
        }
        Err(e) => {
            send_msg(ctx, SendMessage::Error(&e.to_string())).await;
        }
    }

    Ok(())
}
