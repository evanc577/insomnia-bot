#![allow(dead_code)]

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

#[derive(Debug, Deserialize)]
struct Segments {
    segment: (f64, f64),
}

pub async fn get_skips(url: &str) -> Vec<(Duration, Duration)> {
    let id = match get_id(url) {
        Some(id) => id,
        None => return vec![],
    };

    let segments = match get_segments(id).await {
        Some(s) => s,
        None => return vec![],
    };

    postprocess_segments(segments)
        .into_iter()
        .map(|(a, b)| (Duration::from_secs_f64(a), Duration::from_secs_f64(b)))
        .collect()
}

fn get_id(url: &str) -> Option<&str> {
    Some(YT_ID_RE.captures(url)?.get(1)?.as_str())
}

async fn get_segments(id: &str) -> Option<Vec<(f64, f64)>> {
    static URL: &str = "https://sponsor.ajay.app/api/skipSegments";

    let resp = CLIENT
        .get(URL)
        .query(&[
            ("videoID", id),
            ("category", "sponsor"),
            ("category", "selfpromo"),
            ("category", "interaction"),
            ("category", "intro"),
            ("category", "outro"),
            ("category", "preview"),
            ("category", "music_offtopic"),
        ])
        .send()
        .await
        .ok()?;

    let segments = resp
        .json::<Vec<Segments>>()
        .await
        .ok()?
        .into_iter()
        .map(|s| s.segment)
        .collect();

    Some(segments)
}

fn postprocess_segments(mut segments: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    segments.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut ret = vec![];

    let mut cur_segment = (None, None);
    for (start, end) in segments {
        if start >= end {
            continue;
        }
        if let (Some(cur_start), Some(cur_end)) = cur_segment {
            if start > cur_end {
                ret.push((cur_start, cur_end));
                cur_segment = (Some(start), Some(end));
            } else {
                cur_segment.1 = Some(f64::max(cur_end, end));
            }
        } else {
            cur_segment = (Some(start), Some(end));
        }
    }
    if let (Some(cur_start), Some(cur_end)) = cur_segment {
        ret.push((cur_start, cur_end));
    }

    ret
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_skips() {
        let skips = get_skips("https://www.youtube.com/watch?v=Pq_mbTSR-a0").await;
        dbg!(skips);
    }

    #[test]
    fn test_postprocess_segments_nonoverlap() {
        let segments = vec![(0.0, 1.0), (2.0, 3.0), (4.0, 5.0)];
        let expected = vec![(0.0, 1.0), (2.0, 3.0), (4.0, 5.0)];
        assert_eq!(expected, postprocess_segments(segments));
    }

    #[test]
    fn test_postprocess_segments_contain() {
        let segments = vec![(0.0, 5.0), (1.0, 4.0), (2.0, 3.0)];
        let expected = vec![(0.0, 5.0)];
        assert_eq!(expected, postprocess_segments(segments));
    }

    #[test]
    fn test_postprocess_segments_overlap() {
        let segments = vec![(0.0, 3.0), (1.0, 4.0), (2.0, 5.0)];
        let expected = vec![(0.0, 5.0)];
        assert_eq!(expected, postprocess_segments(segments));
    }
}
