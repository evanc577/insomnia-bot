use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use reqwest::header;

use super::ReplacedLink;
use crate::CLIENT;

static REDDIT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bhttps?://(?:(?:www|old|new)\.)?reddit\.com/r/(?P<subreddit>\w+)\b(?:/comments/(?P<submission>\w+\b)(?:/[^/]+/(?P<comment>\w+\b))?)").unwrap()
});
static REDDIT_SHARE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bhttps?://(?:(?:www|old|new)\.)?reddit\.com/r/(?P<subreddit>\w+)\b/s/(?P<id>\w+)")
        .unwrap()
});

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

pub async fn reddit_links(text: &str) -> Vec<ReplacedLink> {
    let mut links = Vec::new();
    links.extend(reddit_normal_links(text));
    links.extend(reddit_share_links(text).await);
    links
}

/// Regular reddit urls
pub fn reddit_normal_links(text: &str) -> Vec<ReplacedLink> {
    let content = REDDIT_RE
        .captures_iter(text)
        .filter_map(|capture| {
            let start = capture.get(0).unwrap().start();
            match_reddit_link(capture).map(|link| ReplacedLink {
                start,
                link: link.url(),
            })
        })
        .collect();
    content
}

/// Reddit app share urls
async fn reddit_share_links(text: &str) -> Vec<ReplacedLink> {
    let share_links = REDDIT_SHARE_RE
        .find_iter(text)
        .map(|m| (m.start(), m.as_str()));

    let links = {
        let mut links: Vec<(usize, Box<str>)> = Vec::new();
        for (start, share_link) in share_links {
            if let Ok(response) = CLIENT
                .head(share_link)
                .header(
                    header::USER_AGENT,
                    header::HeaderValue::from_static("insomnia"),
                )
                .send()
                .await
            {
                links.push((start, response.url().as_str().into()));
            }
        }
        links
    };

    let content = links
        .into_iter()
        .filter_map(|(start, link)| {
            REDDIT_RE
                .captures(&link)
                .and_then(|capture| match_reddit_link(capture))
                .map(|link| ReplacedLink {
                    start,
                    link: link.url(),
                })
        })
        .collect();
    content
}

fn match_reddit_link(m: Captures<'_>) -> Option<RedditLink> {
    match (m.name("subreddit"), m.name("submission"), m.name("comment")) {
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
}
