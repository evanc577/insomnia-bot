use pyo3::prelude::*;
use serde::Deserialize;

use crate::music::error::MusicError;

enum SearchType {
    Song,
    Album,
}

impl SearchType {
    fn result_type(&self) -> impl Iterator<Item = &&'static str> {
        match self {
            SearchType::Song => ["song", "video"].iter(),
            SearchType::Album => ["album"].iter(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YTMusicSearchResult {
    category: String,
    result_type: String,
    video_id: Option<String>,
    browse_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YTMusicAlbum {
    audio_playlist_id: String,
}

pub async fn yt_music_song_search(query: String) -> Result<String, MusicError> {
    let id = search_id(query, SearchType::Song).await?;
    let url = format!("https://music.youtube.com/watch?v={}", id);
    Ok(url)
}

pub async fn yt_music_album_search(query: String) -> Result<String, MusicError> {
    let browse_id = search_id(query, SearchType::Album).await?;
    let playlist_id = playlist_id(browse_id).await?;
    let url = format!("https://www.youtube.com/playlist?list={}", playlist_id);
    Ok(url)
}

/// Search YouTube Music and return the first valid result
async fn search_id(query: String, search_type: SearchType) -> Result<String, MusicError> {
    // Use Python ytmusicapi library
    let search_results_json: Result<anyhow::Result<String>, _> =
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| {
                // Import Python modules
                let ytmusic = PyModule::import(py, "ytmusicapi")?;
                let json = PyModule::import(py, "json")?;

                // Search YouTube Music
                let search = ytmusic
                    .call_method0("YTMusic")?
                    .call_method1("search", (query,))?;

                // Convert to JSON for so Rust can use it
                let search_json = json.call_method1("dumps", (search,))?;
                let ret: String = search_json.extract()?;
                Ok(ret)
            })
        })
        .await;

    let search_results_json: String = search_results_json
        .map_err(|e| MusicError::Internal(e.into()))?
        .map_err(MusicError::Internal)?;

    // Convert JSON to Rust struct
    let search_results: Vec<YTMusicSearchResult> =
        serde_json::from_str(&search_results_json).map_err(|e| MusicError::Internal(e.into()))?;

    // Choose the first result, giving top result priority
    let result = search_results
        .iter()
        .filter(|r| r.category.to_lowercase() == "top result")
        .filter_map(|r| check_results(r, &search_type))
        .chain(
            search_results
                .iter()
                .filter_map(|r| check_results(r, &search_type)),
        )
        .next()
        .ok_or(MusicError::NoResults)?
        .clone();

    Ok(result)
}

fn check_results<'a>(
    result: &'a YTMusicSearchResult,
    search_type: &SearchType,
) -> Option<&'a String> {
    if search_type
        .result_type()
        .any(|&t| t == result.result_type.to_lowercase())
    {
        match search_type {
            SearchType::Song => {
                if let Some(id) = &result.video_id {
                    return Some(id);
                }
            }
            SearchType::Album => {
                if let Some(id) = &result.browse_id {
                    return Some(id);
                }
            }
        }
    }
    None
}

async fn playlist_id(browse_id: String) -> Result<String, MusicError> {
    let album_results_json: Result<anyhow::Result<String>, _> =
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| {
                // Import Python modules
                let ytmusic = PyModule::import(py, "ytmusicapi")?;
                let json = PyModule::import(py, "json")?;

                // Search YouTube Music
                let search = ytmusic
                    .call_method0("YTMusic")?
                    .call_method1("get_album", (browse_id,))?;

                // Convert to JSON for so Rust can use it
                let search_json = json.call_method1("dumps", (search,))?;
                let ret: String = search_json.extract()?;
                Ok(ret)
            })
        })
        .await;

    let album_results_json: String = album_results_json
        .map_err(|e| MusicError::Internal(e.into()))?
        .map_err(MusicError::Internal)?;

    // Convert JSON to Rust struct
    let album_results: YTMusicAlbum =
        serde_json::from_str(&album_results_json).map_err(|e| MusicError::Internal(e.into()))?;

    Ok(album_results.audio_playlist_id)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn song_search() {
        assert!(yt_music_song_search("dreamcatcher deja vu".to_string())
            .await
            .is_ok());
        assert!(yt_music_song_search("a heart of sunflower".to_string())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn album_search() {
        assert!(
            yt_music_album_search("dreamcatcher raid of dream".to_string())
                .await
                .is_ok()
        );
        assert!(
            yt_music_album_search("dreamcatcher summer holiday".to_string())
                .await
                .is_ok()
        );
    }
}
