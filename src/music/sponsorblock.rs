use std::time::Duration;
use tokio::sync::Mutex;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

static CLIENT: Lazy<Mutex<reqwest::Client>> = Lazy::new(|| {
    let client = reqwest::Client::builder().use_rustls_tls().build().unwrap();
    Mutex::new(client)
});

static YT_ID_RE: Lazy<Mutex<Regex>> = Lazy::new(|| {
    let re = Regex::new(r"https:\/\/www\.youtube\.com\/watch\?v=([\w\-]+)").unwrap();
    Mutex::new(re)
});

pub async fn get_skips(url: &str) -> (Option<Duration>, Option<Duration>) {
    (None, None)
}

#[derive(Debug, Deserialize)]
struct Segments {
    segment: (f64, f64),
}

async fn get_id(url: &str) -> Option<&str> {
    Some(YT_ID_RE.lock().await.captures(url)?.get(1)?.as_str())
}

async fn get_segments(id: &str) -> Option<Vec<Segments>> {
    const URL: &str = "https://sponsor.ajay.app/";

    let client = CLIENT.lock().await;
    let resp = client
        .get(URL)
        .query(&["category", "sponsor"])
        .query(&["category", "selfpromo"])
        .query(&["category", "interaction"])
        .query(&["category", "intro"])
        .query(&["category", "outro"])
        .query(&["category", "preview"])
        .query(&["category", "music_offtopic"])
        .send()
        .await
        .ok()?
        .json::<Vec<Segments>>()
        .await
        .ok()?;

    dbg!(&resp);
    Some(resp)
}
