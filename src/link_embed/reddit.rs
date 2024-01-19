use std::sync::Arc;

use itertools::Itertools;
use once_cell::sync::Lazy;
use poise::serenity_prelude::{Http, Message};
use regex::Regex;

static REDDIT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bhttps?://(?:(?:www|old|new)\.)?reddit\.com/r/(?P<subreddit>\w+)\b(?:/comments/(?P<submission>\w+\b)(?:/[^/]+/(?P<comment>\w+\b))?)").unwrap()
});
static REDDIT_SHARE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bhttps?://(?:(?:www|old|new)\.)?reddit\.com/r/(?P<subreddit>\w+)\b/s/(?P<id>\w+)")
        .unwrap()
});

pub async fn send_reddit_embed(http: Arc<Http>, message: Message) {
    let content = parse_reddit(&message.content)
        .map(|link| link.url())
        .join("\n");
    if !content.is_empty() {
        eprintln!(
            "Replying with rxddit link in {}",
            message.channel_id.as_u64()
        );
        message.reply(&http, content).await.unwrap();
    }
}

enum RedditLink {
    Submission {
        id: Box<str>,
    },
    Comment {
        subreddit: Box<str>,
        submission_id: Box<str>,
        comment_id: Box<str>,
    },
}

impl RedditLink {
    fn url(&self) -> Box<str> {
        match self {
            RedditLink::Submission { id } => format!("https://rxddit.com/{id}").into_boxed_str(),
            RedditLink::Comment {
                subreddit,
                submission_id,
                comment_id,
            } => {
                format!("https://rxddit.com/r/{subreddit}/comments/{submission_id}/_/{comment_id}")
                    .into_boxed_str()
            }
        }
    }
}

fn parse_reddit(text: &str) -> impl Iterator<Item = RedditLink> + '_ {
    let content = REDDIT_RE.captures_iter(text).filter_map(|capture| {
        match (
            capture.name("subreddit"),
            capture.name("submission"),
            capture.name("comment"),
        ) {
            (Some(subreddit), Some(submission_id), Some(comment_id)) => Some(RedditLink::Comment {
                subreddit: subreddit.as_str().into(),
                submission_id: submission_id.as_str().into(),
                comment_id: comment_id.as_str().into(),
            }),
            (_, Some(submission_id), _) => Some(RedditLink::Submission {
                id: submission_id.as_str().into(),
            }),
            _ => None,
        }
    });
    content
}
