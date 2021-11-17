use pyo3::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YTMusicSearchResult {
    category: String,
    result_type: String,
    title: Option<String>,
    video_id: Option<String>,
}

pub async fn yt_music_search(query: String) -> Option<String> {
    let id = yt_music_search_id(query).await;
    if let Some(id) = id {
        return Some(format!("https://music.youtube.com/watch?v={}", id));
    }
    None
}

async fn yt_music_search_id(query: String) -> Option<String> {
    // Use Python ytmusicapi library
    let search_results_json = tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // Import Python modules
            let ytmusic = PyModule::import(py, "ytmusicapi").ok()?;
            let json = PyModule::import(py, "json").ok()?;

            // Search YouTube Music
            let search = ytmusic.call_method0("YTMusic").ok()?.call_method1("search", (query,)).ok()?;

            // Convert to JSON for so Rust can use it
            let search_json = json.call_method1("dumps", (search,)).ok()?;
            let ret: String = search_json.extract().ok()?;
            Some(ret)
        })
    }).await;

    let search_results_json = if let Ok(Some(x)) = search_results_json {
        x
    } else {
        return None;
    };

    // Convert JSON to Rust struct
    let search_results: Vec<YTMusicSearchResult> = serde_json::from_str(&search_results_json).ok()?;

    // Check top results
    for result in search_results.iter().filter(|r| r.category.to_lowercase() == "top result") {
        if result.result_type.to_lowercase() == "song" {
            if result.video_id.is_some() {
                return result.video_id.clone();
            }
        }
    }

    // Check songs
    for result in search_results.iter().filter(|r| r.category.to_lowercase() == "songs") {
        if result.result_type.to_lowercase() == "song" {
            if result.video_id.is_some() {
                return result.video_id.clone();
            }
        }
    }

    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn music_search() {
        assert!(yt_music_search("dreamcatcher deja vu".to_string()).await.is_some());
        assert!(yt_music_search("a heart of sunflower".to_string()).await.is_some());
    }
}
