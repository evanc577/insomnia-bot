use if_chain::if_chain;
use songbird::tracks::{PlayMode, TrackHandle};

use super::error::MusicError;
use super::message::PlayUpdate;
use super::queue::{add_tracks, remove_track, Query};
use super::voice::{CanGetVoice, CanJoinVoice};
use super::youtube::music::{yt_music_album_search, yt_music_song_search};
use super::youtube::music_autocomplete::autocomplete_ytmusic;
use super::youtube::playlist::add_youtube_playlist;
use crate::message::{CustomSendMessage, SendMessage, SendableMessage};
use crate::{PoiseContext, PoiseError};

/// Play a song via YouTube Music or URL, if no argument is given, resume the paused track
#[poise::command(slash_command, prefix_command, guild_only, broadcast_typing)]
pub async fn play(
    ctx: PoiseContext<'_>,
    #[rest]
    #[description = "Song title or URL"]
    #[rename = "song_or_url"]
    #[autocomplete = "autocomplete_ytmusic"]
    arg: Option<String>,
) -> Result<(), PoiseError> {
    if let Some(arg) = arg {
        ctx.defer_or_broadcast().await?;
        if let Ok(url) = url::Url::parse(&arg) {
            // Try parsing url as youtube playlist
            if add_youtube_playlist(ctx, url.as_str()).await.is_ok() {
                return Ok(());
            }
            // Try adding url as a track
            let query = Query::Url(url.to_string());
            add_tracks(ctx, futures::stream::once(async { query }), 1).await?;
        } else {
            // Otherwise search YouTube Music
            search_add_song(&ctx, arg).await?;
        }
    } else {
        // If no arguments, resume current track
        let handler_lock = ctx.get_voice().await?;
        if_chain! {
            let handler = handler_lock.lock().await;
            let queue = handler.queue();
            if let Some(track) = queue.current();
            if let Ok(info) = track.get_info().await;
            if info.playing == PlayMode::Pause;
            then {
                let _ = track.play();
            } else {
                return Err(MusicError::NoPausedTrack.into());
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
) -> Result<(), PoiseError> {
    search_add_song(&ctx, arg).await
}

async fn search_add_song(ctx: &PoiseContext<'_>, song: String) -> Result<(), PoiseError> {
    ctx.defer_or_broadcast().await?;
    let url = yt_music_song_search(song).await?;
    let query = Query::Url(url.to_string());
    add_tracks(*ctx, futures::stream::once(async { query }), 1).await?;

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
) -> Result<(), PoiseError> {
    ctx.defer_or_broadcast().await?;
    let query = if let Ok(url) = url::Url::parse(&arg) {
        // If URL is given, play URL
        Query::Url(url.to_string())
    } else {
        // Otherwise search YouTube Music
        Query::Search(arg)
    };
    add_tracks(ctx, futures::stream::once(async { query }), 1).await?;

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
) -> Result<(), PoiseError> {
    // Otherwise search YouTube Music
    ctx.defer_or_broadcast().await?;
    let url = yt_music_album_search(arg).await?;
    add_youtube_playlist(ctx, url.as_str()).await?;

    Ok(())
}

/// Pause the currently playing track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn pause(ctx: PoiseContext<'_>) -> Result<(), PoiseError> {
    let handler_lock = ctx.join_voice().await?;
    let handler = handler_lock.lock().await;
    let queue = handler.queue();
    let track = queue.current().ok_or(MusicError::NoPlayingTrack)?;
    let info = track
        .get_info()
        .await
        .map_err(|e| MusicError::Internal(e.into()))?;
    if info.playing == PlayMode::Play {
        track.pause()?;
        CustomSendMessage::Custom(PlayUpdate::Pause(track.clone()).format().await)
            .send_msg(ctx)
            .await;
    }

    Ok(())
}

/// Skip the currently playing track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn skip(ctx: PoiseContext<'_>) -> Result<(), PoiseError> {
    let handler_lock = ctx.get_voice().await?;
    let handler = handler_lock.lock().await;
    let track = handler
        .queue()
        .dequeue(0)
        .ok_or(MusicError::NoPlayingTrack)?;
    let _ = track.stop();
    CustomSendMessage::Custom(PlayUpdate::Skip(track.clone()).format().await)
        .send_msg(ctx)
        .await;
    if let Some(next) = handler.queue().current() {
        let _ = next.play();
    }

    Ok(())
}

/// Stop playing and clear queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn stop(ctx: PoiseContext<'_>) -> Result<(), PoiseError> {
    let handler_lock = ctx.get_voice().await?;
    let handler = handler_lock.lock().await;
    let queue = handler.queue();
    if let Some(track) = queue.current() {
        let _ = track.stop();
    }
    queue.stop();
    CustomSendMessage::Custom(PlayUpdate::Stop.format().await)
        .send_msg(ctx)
        .await;

    Ok(())
}

/// List tracks in queue
#[poise::command(slash_command, prefix_command, guild_only, aliases("ls", "queue"))]
pub async fn list(
    ctx: PoiseContext<'_>,
    #[rename = "index"]
    #[description = "start listing from this index in queue, use \"end\" to list end of queue"]
    arg: Option<String>,
) -> Result<(), PoiseError> {
    const NUM_TRACKS: usize = 25;

    let handler_lock = ctx.get_voice().await?;
    let handler = handler_lock.lock().await;
    let queue = handler.queue().current_queue();

    // Parse arguments
    let arg = match arg {
        Some(a) => a,
        None => "1".to_owned(),
    };
    let start = match arg.to_lowercase().as_str() {
        "max" => None,
        "end" => None,
        s => Some(s.parse::<usize>().map_err(|_| MusicError::BadIndex)?),
    };

    // Compute start index
    let start = if let Some(i) = start {
        i
    } else {
        std::cmp::max(1, queue.len().saturating_sub(NUM_TRACKS))
    };
    if start == 0 {
        return Err(MusicError::BadIndex.into());
    }

    let format_track = |track: &TrackHandle| {
        let title = track
            .metadata()
            .title
            .clone()
            .unwrap_or_else(|| "Unknown".into());

        title.replace('`', "")
    };

    // Build output string
    let list = queue
        .iter()
        .skip(start - 1)
        .zip(start..start + NUM_TRACKS)
        .map(|(t, i)| format!("{:>2}: {}", i, format_track(t)))
        .collect::<Vec<_>>()
        .join("\n");

    // Respond
    let out_msg = match queue.len() {
        0 => "Queue is empty".to_owned(),
        1 => format!("{} track in queue\n```{}```", 1, list),
        n => format!("{} tracks in queue\n```{}```", n, list),
    };
    SendMessage::Normal(&out_msg).send_msg(ctx).await;

    Ok(())
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
) -> Result<(), PoiseError> {
    // Parse arguments
    let start_idx = track - 1;
    let end_idx = match track_end {
        Some(i) => i - 1,
        None => start_idx,
    };

    // Remove tracks
    let removed = remove_track(ctx, start_idx, end_idx).await?;
    if removed.len() == 1 {
        CustomSendMessage::Custom(PlayUpdate::Remove(removed[0].clone()).format().await)
            .send_msg(ctx)
            .await;
    } else {
        SendMessage::Normal(&format!("Removed {} tracks", removed.len()))
            .send_msg(ctx)
            .await;
    }

    Ok(())
}
