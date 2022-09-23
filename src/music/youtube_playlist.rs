use std::process::Command;

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

use super::error::MusicError;
use crate::music::queue::{add_tracks, Query};
use crate::PoiseContext;

static YT_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://(www|music)\.youtube\.com/playlist\?list=(?P<id>[0-9a-zA-Z\-_]+)")
        .unwrap()
});

#[derive(Debug, Deserialize)]
struct PlaylistTrack {
    id: String,
}

pub async fn add_youtube_playlist(ctx: PoiseContext<'_>, url: &str) -> Result<Option<usize>> {
    let id = match playlist_id(url) {
        Some(id) => id,
        None => return Ok(None),
    };
    let tracks = get_playlist_tracks(id).await?;
    let num_tracks = tracks.len();

    let urls: Vec<_> = tracks
        .iter()
        .map(|t| Query::Url(format!("https://www.youtube.com/watch?v={}", t.id)))
        .collect();
    let _ = add_tracks(ctx, urls).await;

    Ok(Some(num_tracks))
}

fn playlist_id(url: &str) -> Option<&str> {
    Some(YT_ID_RE.captures(url)?.name("id")?.as_str())
}

async fn get_playlist_tracks(playlist_id: &str) -> Result<Vec<PlaylistTrack>> {
    let playlist_url = format!("https://www.youtube.com/playlist?list={}", playlist_id);

    // Get playlist items via youtube-dl
    let output = tokio::spawn(async move {
        match Command::new("yt-dlp")
            .arg("--ignore-config")
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg(playlist_url)
            .output()
        {
            Ok(o) => {
                if !o.status.success() {
                    return None;
                }
                Some(String::from_utf8_lossy(&o.stdout).into_owned())
            }
            Err(_) => None,
        }
    })
    .await;
    let output = if let Ok(Some(o)) = output {
        o
    } else {
        return Err(MusicError::BadPlaylist.into());
    };

    // Parse playlist
    let playlist_tracks: Vec<_> = output
        .split('\n')
        .filter_map(|l| serde_json::from_str::<PlaylistTrack>(l).ok())
        .collect();

    Ok(playlist_tracks)
}
