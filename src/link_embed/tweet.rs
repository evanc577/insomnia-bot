use once_cell::sync::Lazy;
use regex::Regex;

use super::ReplacedLink;

static TWEET_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bhttps?://(twitter|x)\.com/(?P<tweet>\w+/status/\d+)\b").unwrap());

pub fn tweet_links(text: &str) -> Vec<ReplacedLink> {
    TWEET_RE
        .captures_iter(text)
        .map(|capture| capture.name("tweet").unwrap())
        .map(|m| (m.start(), m.as_str()))
        .map(|(start, tweet)| ReplacedLink {
            start,
            link: format!("https://vxtwitter.com/{}", tweet).into(),
            media: None,
        })
        .collect()
}
