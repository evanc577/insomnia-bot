use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

use super::UrlReplacer;

impl UrlReplacer {
    pub async fn replace_dubz(url: &url::Url) -> Vec<Url> {
        static PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/c/(?P<id>\w+)").unwrap());
        let url = PATH_RE
            .captures(url.path())
            .map(|cap| cap.name("id").unwrap())
            .and_then(|id| {
                Url::parse(&format!(
                    "https://dubzalt.com/storage/videos/{}.mp4",
                    id.as_str()
                ))
                .ok()
            });
        if let Some(url) = url {
            vec![url]
        } else {
            Vec::new()
        }
    }
}
