use std::sync::Arc;

use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use reqwest::{header, Client};
use serde::{Deserialize, Deserializer, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

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
    static REDDIT_ACCESS_TOKEN: Lazy<AccessToken> = Lazy::new(AccessToken::default);
    static REDDIT_USER_AGENT: &str = "Reddit";

    let share_links = REDDIT_SHARE_RE
        .find_iter(text)
        .map(|m| (m.start(), m.as_str()));

    let links = {
        let mut links: Vec<(usize, Box<str>)> = Vec::new();
        for (start, share_link) in share_links {
            let mut headers = header::HeaderMap::new();
            headers.insert(
                header::USER_AGENT,
                header::HeaderValue::from_static(REDDIT_USER_AGENT),
            );
            if let Some(auth) = REDDIT_ACCESS_TOKEN.authentication(&CLIENT).await {
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&auth).unwrap(),
                );
            }

            if let Ok(response) = CLIENT.head(share_link).headers(headers).send().await {
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

// Use an access token to bypass rate limits
#[derive(Default, Clone)]
struct AccessToken {
    token: Arc<Mutex<Option<AccessTokenInternal>>>,
}

impl AccessToken {
    /// Return stored authorization, refresh it if needed
    async fn authentication(&self, client: &Client) -> Option<String> {
        let mut access_token_guard = self.token.lock().await;
        if access_token_guard.is_none() {
            // Get a new token if none exists
            let new_token = AccessTokenInternal::access_token(client).await?;
            *access_token_guard = Some(new_token);
        } else if let Some(token) = &*access_token_guard {
            // Get a new token if current one is expiring soon
            let expiry = token.expiry;
            let buffer_time = Duration::new(4 * 60 * 60, 0); // 4 hours
            if expiry - OffsetDateTime::now_utc() < buffer_time {
                let new_token = AccessTokenInternal::access_token(client).await?;
                *access_token_guard = Some(new_token);
            }
        }
        Some(format!(
            "Bearer {}",
            access_token_guard.as_ref().unwrap().access_token.clone()
        ))
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct AccessTokenInternal {
    access_token: String,
    #[serde(rename = "expiry_ts")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    expiry: OffsetDateTime,
    #[serde(deserialize_with = "deserialize_duration")]
    expires_in: Duration,
    scope: Vec<String>,
    token_type: String,
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let ts = i64::deserialize(deserializer)?;
    let dt = OffsetDateTime::from_unix_timestamp(ts).map_err(serde::de::Error::custom)?;
    Ok(dt)
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s = i64::deserialize(deserializer)?;
    let d = Duration::new(s, 0);
    Ok(d)
}

#[derive(Serialize)]
struct Body {
    scopes: Vec<String>,
}

impl AccessTokenInternal {
    async fn access_token(client: &Client) -> Option<Self> {
        static ENDPOINT: &str = "https://accounts.reddit.com/api/access_token";
        static AUTHORIZATION: &str = "basic b2hYcG9xclpZdWIxa2c6";
        let body = Body {
            scopes: vec!["*".into(), "email".into(), "pii".into()],
        };
        let response = client
            .post(ENDPOINT)
            .header(header::AUTHORIZATION, AUTHORIZATION)
            .json(&body)
            .send()
            .await
            .ok()?;
        let status = response.status();
        if status.is_success() {
            Some(response.json().await.ok()?)
        } else {
            None
        }
    }
}
