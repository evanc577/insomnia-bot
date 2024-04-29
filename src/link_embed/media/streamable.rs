use once_cell::sync::Lazy;
use regex::Regex;

use super::MediaLinkResolver;

pub struct StreamableMediaResolver;

impl MediaLinkResolver for StreamableMediaResolver {
    async fn resolve(url: &reqwest::Url) -> Option<Box<str>> {
        static PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/(?P<id>\w+)").unwrap());
        PATH_RE
            .captures(url.path())
            .map(|cap| cap.name("id").unwrap())
            .map(|id| {
                format!("https://streamable.com/{}", id.as_str()).into_boxed_str()
            })
    }
}
