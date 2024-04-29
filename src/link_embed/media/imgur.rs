use once_cell::sync::Lazy;
use regex::Regex;

use super::MediaLinkResolver;

pub struct ImgurMediaResolver;

impl MediaLinkResolver for ImgurMediaResolver {
    async fn resolve(url: &reqwest::Url) -> Option<Box<str>> {
        static PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/(?P<id>\w+)").unwrap());
        PATH_RE
            .captures(url.path())
            .map(|cap| cap.name("id").unwrap())
            .map(|id| {
                format!("https://i.imgur.com/{}.mp4", id.as_str()).into_boxed_str()
            })
    }
}
