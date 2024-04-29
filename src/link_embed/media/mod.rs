use self::dubz::DubzMediaResolver;
use self::imgur::ImgurMediaResolver;
use self::streamable::StreamableMediaResolver;

mod dubz;
mod streamable;
mod imgur;

pub trait MediaLinkResolver {
    async fn resolve(url: &reqwest::Url) -> Option<Box<str>>;
}

pub(crate) async fn resolve_media_link(url: &reqwest::Url) -> Option<Box<str>> {
    match url.domain() {
        Some("streamable.com") => StreamableMediaResolver::resolve(url).await,
        Some("dubz.link") => DubzMediaResolver::resolve(url).await,
        Some("imgur.com") | Some("i.imgur.com") => ImgurMediaResolver::resolve(url).await,
        _ => None,
    }
}
