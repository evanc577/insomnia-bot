use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

use super::UrlReplacer;

impl UrlReplacer {
    pub async fn replace_tweet(url: &Url) -> Vec<Url> {
        static TWEET_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\bhttps?://(twitter|x)\.com/(?P<tweet>\w+/status/\d+)\b").unwrap()
        });

        TWEET_RE
            .captures_iter(url.as_str())
            .filter_map(|capture| {
                let path = capture.name("tweet").unwrap().as_str();
                let url = format!("https://vxtwitter.com/{}", path);
                Url::parse(&url).ok()
            })
            .collect()
    }
}
