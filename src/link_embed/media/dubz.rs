use once_cell::sync::Lazy;
use regex::Regex;

use super::MediaLinkResolver;

pub struct DubzMediaResolver;

impl MediaLinkResolver for DubzMediaResolver {
    async fn resolve(url: &reqwest::Url) -> Option<Box<str>> {
        static PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/c/(?P<id>\w+)").unwrap());
        PATH_RE
            .captures(url.path())
            .map(|cap| cap.name("id").unwrap())
            .map(|id| {
                format!("https://dubzalt.com/storage/videos/{}.mp4", id.as_str()).into_boxed_str()
            })
    }
}
