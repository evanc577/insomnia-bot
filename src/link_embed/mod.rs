mod reddit;
mod tweet;
mod media;

use std::sync::Arc;

use itertools::Itertools;
use poise::serenity_prelude::{Http, Message, SerenityError};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ReplacedLink {
    start: usize,
    link: Box<str>,
    media: Option<Box<str>>,
}

impl ReplacedLink {
    fn url(&self) -> Box<str> {
        self.link.clone()
    }
}

#[derive(Debug)]
pub struct LinkEmbedError {
    context: String,
    inner: SerenityError,
}

impl std::fmt::Display for LinkEmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "context: {}, {}", self.context, self.inner)
    }
}

impl std::error::Error for LinkEmbedError {}

pub async fn reply_link_embeds(
    http: Arc<Http>,
    mut message: Message,
) -> Result<(), LinkEmbedError> {
    let tweet_links = tweet::tweet_links(&message.content);
    let reddit_links = reddit::reddit_links(&message.content).await;
    let reply_content = tweet_links
        .into_iter()
        .chain(reddit_links.into_iter())
        .sorted()
        .map(|link| {
            let mut s = link.url().to_string();
            if let Some(media) = link.media {
                s.push_str(&format!("\n{}", &media));
            }
            s
        })
        .join("\n");
    if !reply_content.is_empty() {
        eprintln!(
            "Replying with updated link in {}",
            message.channel_id.as_u64()
        );
        message
            .reply(&http, reply_content)
            .await
            .map_err(|e| LinkEmbedError {
                context: "Send message".into(),
                inner: e,
            })?;
        message
            .suppress_embeds(http)
            .await
            .map_err(|e| LinkEmbedError {
                context: "Suppress original embeds".into(),
                inner: e,
            })?;
    }
    Ok(())
}
