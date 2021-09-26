use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
});

static JSON_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<script.*?ytInitialPlayerResponse.*?(\{.*\}).*</script").unwrap());

static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://www\.youtube\.com").unwrap());

pub async fn get_loudness(url: &str) -> Option<f32> {
    if !URL_RE.is_match(url) {
        return None;
    }

    let loudness_db = query_youtube_db(url).await?;
    Some(db_to_float(loudness_db))
}

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

async fn query_youtube_db(url: &str) -> Option<f32> {
    // Query YouTube
    let text = {
        let resp = CLIENT.get(url).send().await.ok()?.text().await.ok()?;
        resp
    };

    // Extract JSON string
    let json_str = {
        let caps = JSON_RE.captures(&text)?;
        let m = caps.get(1)?;
        m.as_str()
    };

    // Parse JSON
    let resp: YTInitialPlayerResponse = serde_json::from_str(json_str).ok()?;

    Some(resp.player_config.audio_config.loudness_db)
}

fn db_to_float(db: f32) -> f32 {
    if db < 0.0 {
        return 1.0;
    }
    let raw_percent = 10f32.powf(-1.0 * db / 20.0);
    if !raw_percent.is_finite() {
        return 1.0;
    }
    raw_percent.clamp(0.0, 1.0)
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::*;

    static URL: &str = "https://www.youtube.com/watch?v=5gvfp-haKXc";

    #[tokio::test]
    async fn check_db() {
        let result = query_youtube_db(URL).await;
        assert!(result.is_some());
        if let Some(x) = result {
            assert!(abs_diff_eq!(5.63, x, epsilon = 0.001));
        }
    }
}