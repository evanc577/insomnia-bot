use std::process::Command;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serenity::{client::Context, model::channel::Message};

use crate::music::queue::{add_track, Query};

static YT_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://(www|music)\.youtube\.com/playlist\?list=(?P<id>[0-9a-zA-Z\-_]+)")
        .unwrap()
});

#[derive(Debug, Deserialize)]
struct PlaylistTrack {
    id: String,
    title: String,
    playlist_title: Option<String>,
    playlist_index: Option<usize>,
}

pub async fn add_youtube_playlist(ctx: &Context, msg: &Message, url: &str) -> Option<usize> {
    let id = playlist_id(url)?;
    let tracks = get_playlist_tracks(id).await;
    let num_tracks = tracks.len();

    for track in tracks {
        let url = format!("https://www.youtube.com/watch?v={}", track.id);
        let _ = add_track(ctx, msg, Query::URL(&url)).await;
    }

    Some(num_tracks)
}

fn playlist_id(url: &str) -> Option<&str> {
    Some(YT_ID_RE.captures(url)?.name("id")?.as_str())
}

async fn get_playlist_tracks(playlist_id: &str) -> Vec<PlaylistTrack> {
    let playlist_url = format!("https://www.youtube.com/playlist?list={}", playlist_id);

    // Get playlist items via youtube-dl
    let output = tokio::spawn(async move {
        match Command::new("youtube-dl")
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
            Err(_) => return None,
        }
    })
    .await;
    let output = if let Ok(Some(o)) = output {
        o
    } else {
        return vec![];
    };

    // Parse playlist
    let playlist_tracks: Vec<_> = output
        .split("\n")
        .filter_map(|l| serde_json::from_str::<PlaylistTrack>(l).ok())
        .collect();

    playlist_tracks
}
