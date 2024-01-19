mod reddit;
mod tweet;

use std::sync::Arc;

use itertools::Itertools;
use poise::serenity_prelude::{Http, Message};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ReplacedLink {
    start: usize,
    link: Box<str>,
}

impl ReplacedLink {
    fn url(&self) -> Box<str> {
        self.link.clone()
    }
}

pub async fn reply_link_embeds(http: Arc<Http>, message: Message) {
    let tweet_links = tweet::tweet_links(&message.content);
    let reddit_links = reddit::reddit_links(&message.content).await;
    let reply_content = tweet_links
        .into_iter()
        .chain(reddit_links.into_iter())
        .sorted()
        .map(|link| link.url())
        .join("\n");
    if !reply_content.is_empty() {
        eprintln!(
            "Replying with updated link in {}",
            message.channel_id.as_u64()
        );
        message.reply(&http, reply_content).await.unwrap();
    }
}
