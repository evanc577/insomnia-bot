#![allow(dead_code)]
#![allow(unused_variables)]

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

static YT_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https://www\.youtube\.com/watch\?v=([\w\-]+)").unwrap());

pub async fn get_skips(url: &str) -> Vec<(Duration, Duration)> {
    vec![]
}

#[derive(Debug, Deserialize)]
struct Segments {
    segment: (f64, f64),
}

async fn get_id(url: &str) -> Option<&str> {
    Some(YT_ID_RE.captures(url)?.get(1)?.as_str())
}

async fn get_segments(id: &str) -> Option<Vec<Segments>> {
    const URL: &str = "https://sponsor.ajay.app/";

    let resp = CLIENT
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
