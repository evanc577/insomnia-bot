use self::dubz::DubzMediaResolver;

mod dubz;

pub trait MediaLinkResolver {
    async fn resolve(url: &reqwest::Url) -> Option<Box<str>> {
        Some(url.as_str().into())
    }
}

struct DefaultMediaResolver;

impl MediaLinkResolver for DefaultMediaResolver {}

pub(crate) async fn resolve_media_link(url: &reqwest::Url) -> Option<Box<str>> {
    match url.domain() {
        Some("dubz.link") => DubzMediaResolver::resolve(url).await,
        Some("imgur.com") | Some("i.imgur.com") => DefaultMediaResolver::resolve(url).await,
        Some("streamable.com") => DefaultMediaResolver::resolve(url).await,
        _ => None,
    }
}
