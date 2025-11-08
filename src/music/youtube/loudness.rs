use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;
use serde::Deserialize;

use crate::music::error::MusicError;
use crate::CLIENT;

static JSON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<script.*?ytInitialPlayerResponse.*?(\{.*\}).*</script").unwrap()
});

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://www\.youtube\.com").unwrap());

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YTInitialPlayerResponse {
    player_config: PlayerConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerConfig {
    audio_config: AudioConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioConfig {
    loudness_db: f32,
}

pub async fn get_loudness(url: &str) -> f32 {
    get_loudness_helper(url).await.unwrap_or(1.0)
}

async fn get_loudness_helper(url: &str) -> Result<f32> {
    if !URL_RE.is_match(url) {
        return Err(MusicError::Loudness.into());
    }

    let loudness_db = query_youtube_db(url).await?;
    Ok(db_to_float(loudness_db))
}

async fn query_youtube_db(url: &str) -> Result<f32> {
    // Query YouTube
    let text = { CLIENT.get(url).send().await?.text().await? };

    // Extract JSON string
    let json_str = {
        let caps = JSON_RE.captures(&text).ok_or(MusicError::Loudness)?;
        let m = caps.get(1).ok_or(MusicError::Loudness)?;
        m.as_str()
    };

    // Parse JSON
    let resp: YTInitialPlayerResponse = serde_json::from_str(json_str)?;

    Ok(resp.player_config.audio_config.loudness_db)
}

fn db_to_float(db: f32) -> f32 {
    if db < 0.0 {
        return 1.0;
    }
    let raw_percent = 10f32.powf(-db / 20.0);
    if !raw_percent.is_finite() {
        return 1.0;
    }
    raw_percent.clamp(0.0, 1.0)
}

#[cfg(test)]
mod test {
    use approx::*;

    use super::*;

    static URL: &str = "https://www.youtube.com/watch?v=5gvfp-haKXc";

    #[tokio::test]
    async fn check_db() {
        let result = query_youtube_db(URL).await;
        assert!(result.is_ok());
        if let Ok(x) = result {
            assert!(abs_diff_eq!(5.63, x, epsilon = 0.001));
        }
    }
}
