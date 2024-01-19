use std::sync::Arc;

use itertools::Itertools;
use once_cell::sync::Lazy;
use poise::serenity_prelude::{Http, Message};
use regex::Regex;

static TWEET_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bhttps?://(twitter|x)\.com/(?P<tweet>\w+/status/\d+)\b").unwrap());

pub async fn send_tweet_embed(http: Arc<Http>, message: Message) {
    let content = TWEET_RE
        .captures_iter(&message.content)
        .map(|capture| capture.name("tweet").unwrap().as_str())
        .map(|tweet| format!("https://fxtwitter.com/{}", tweet))
        .join("\n");
    if !content.is_empty() {
        eprintln!(
            "Replying with fxtwitter link in {}",
            message.channel_id.as_u64()
        );
        message.reply(&http, content).await.unwrap();
    }
}
