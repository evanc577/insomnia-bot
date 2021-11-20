use pyo3::prelude::*;
use serde::Deserialize;

enum SearchType {
    Song,
    Album,
}

impl SearchType {
    fn result_type(&self) -> &str {
        match self {
            SearchType::Song => "song",
            SearchType::Album => "album",
        }
    }

    fn category(&self) -> &str {
        match self {
            SearchType::Song => "songs",
            SearchType::Album => "albums",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YTMusicSearchResult {
    category: String,
    result_type: String,
    title: Option<String>,
    video_id: Option<String>,
    browse_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YTMusicAlbum {
    audio_playlist_id: String,
}

pub async fn yt_music_song_search(query: String) -> Option<String> {
    let id = search_id(query, SearchType::Song).await;
    if let Some(id) = id {
        return Some(format!("https://music.youtube.com/watch?v={}", id));
    }
    None
}

pub async fn yt_music_album_search(query: String) -> Option<String> {
    let browse_id = search_id(query, SearchType::Album).await?;
    let playlist_id = playlist_id(browse_id).await;
    if let Some(id) = playlist_id {
        return Some(format!("https://www.youtube.com/playlist?list={}", id));
    }
    None
}

async fn search_id(query: String, search_type: SearchType) -> Option<String> {
    // Use Python ytmusicapi library
    let search_results_json = tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // Import Python modules
            let ytmusic = PyModule::import(py, "ytmusicapi").ok()?;
            let json = PyModule::import(py, "json").ok()?;

            // Search YouTube Music
            let search = ytmusic
                .call_method0("YTMusic")
                .ok()?
                .call_method1("search", (query,))
                .ok()?;

            // Convert to JSON for so Rust can use it
            let search_json = json.call_method1("dumps", (search,)).ok()?;
            let ret: String = search_json.extract().ok()?;
            Some(ret)
        })
    })
    .await;

    let search_results_json = if let Ok(Some(x)) = search_results_json {
        x
    } else {
        return None;
    };

    // Convert JSON to Rust struct
    let search_results: Vec<YTMusicSearchResult> =
        serde_json::from_str(&search_results_json).ok()?;

    // Check top results
    for result in search_results
        .iter()
        .filter(|r| r.category.to_lowercase() == "top result")
    {
        if result.result_type.to_lowercase() == search_type.result_type() {
            match search_type {
                SearchType::Song => {
                    if result.video_id.is_some() {
                        return result.video_id.clone();
                    }
                }
                SearchType::Album => {
                    if result.browse_id.is_some() {
                        return result.browse_id.clone();
                    }
                }
            }
        }
    }

    // Check songs
    for result in search_results
        .iter()
        .filter(|r| r.category.to_lowercase() == search_type.category())
    {
        if result.result_type.to_lowercase() == search_type.result_type() {
            match search_type {
                SearchType::Song => {
                    if result.video_id.is_some() {
                        return result.video_id.clone();
                    }
                }
                SearchType::Album => {
                    if result.browse_id.is_some() {
                        return result.browse_id.clone();
                    }
                }
            }
        }
    }

    None
}

async fn playlist_id(browse_id: String) -> Option<String> {
    let album_results_json = tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // Import Python modules
            let ytmusic = PyModule::import(py, "ytmusicapi").ok()?;
            let json = PyModule::import(py, "json").ok()?;

            // Search YouTube Music
            let search = ytmusic
                .call_method0("YTMusic")
                .ok()?
                .call_method1("get_album", (browse_id,))
                .ok()?;

            // Convert to JSON for so Rust can use it
            let search_json = json.call_method1("dumps", (search,)).ok()?;
            let ret: String = search_json.extract().ok()?;
            Some(ret)
        })
    })
    .await;

    let album_results_json = if let Ok(Some(x)) = album_results_json {
        x
    } else {
        return None;
    };

    // Convert JSON to Rust struct
    let album_results: YTMusicAlbum = serde_json::from_str(&album_results_json).ok()?;

    Some(album_results.audio_playlist_id)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn song_search() {
        assert!(yt_music_song_search("dreamcatcher deja vu".to_string())
            .await
            .is_some());
        assert!(yt_music_song_search("a heart of sunflower".to_string())
            .await
            .is_some());
    }

    #[tokio::test]
    async fn album_search() {
        assert!(
            yt_music_album_search("dreamcatcher raid of dream".to_string())
                .await
                .is_some()
        );
        assert!(
            yt_music_album_search("dreamcatcher summer holiday".to_string())
                .await
                .is_some()
        );
    }
}
