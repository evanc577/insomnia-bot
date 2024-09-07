use std::sync::Arc;

use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use reqwest::{header, Client};
use serde::{Deserialize, Deserializer, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
use url::Url;

use super::UrlReplacer;
use crate::CLIENT;

impl UrlReplacer {
    pub async fn replace_reddit(url: &Url) -> Vec<Url> {
        let url = resolve_link_redirect(url).await;
        reddit_normal_links(url).await.into_iter().collect()
    }
}

static REDDIT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bhttps?://(?:(?:www|old|new|sh)\.)?reddit\.com/r/(?P<subreddit>\w+)?\b(?:/comments/(?P<submission>\w+\b)(?:/[^/]+/(?P<comment>\w+\b))?)").unwrap()
});

static REDDIT_ACCESS_TOKEN: Lazy<AccessToken> = Lazy::new(AccessToken::default);
static REDDIT_USER_AGENT: &str = "Reddit";

enum RedditLink {
    Submission {
        subreddit: Box<str>,
        id: Box<str>,
        media: Option<Url>,
    },
    Comment {
        submission_id: Box<str>,
        comment_id: Box<str>,
    },
}

impl RedditLink {
    fn url(&self) -> Option<Url> {
        let url = match self {
            RedditLink::Submission { id, subreddit, .. } => {
                format!("https://rxddit.com/r/{subreddit}/comments/{id}")
            }
            RedditLink::Comment {
                submission_id,
                comment_id,
            } => format!("https://rxddit.com/comments/{submission_id}/_/{comment_id}"),
        };
        Url::parse(&url).ok()
    }

    fn media(&self) -> Option<Url> {
        match self {
            RedditLink::Submission { media, .. } => media.clone(),
            _ => None,
        }
    }
}

/// Regular reddit urls
async fn reddit_normal_links(url: Option<Url>) -> impl IntoIterator<Item = Url> {
    let mut replaced_urls = Vec::new();
    if let Some(url) = url {
        if let Some(capture) = REDDIT_RE.captures(url.as_str()) {
            if let Some(reddit_link) = match_reddit_link(capture).await {
                if let Some(url) = reddit_link.url() {
                    replaced_urls.push(url);
                }
                if let Some(media) = reddit_link.media() {
                    replaced_urls.push(media);
                }
            }
        }
    }
    replaced_urls
}

async fn resolve_link_redirect(url: &Url) -> Option<Url> {
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

    if let Ok(response) = CLIENT.head(url.as_str()).headers(headers).send().await {
        Url::parse(response.url().as_str()).ok()
    } else {
        None
    }
}

async fn match_reddit_link(m: Captures<'_>) -> Option<RedditLink> {
    match (m.name("subreddit"), m.name("submission"), m.name("comment")) {
        (_, Some(submission_id), Some(comment_id)) => Some(RedditLink::Comment {
            submission_id: submission_id.as_str().into(),
            comment_id: comment_id.as_str().into(),
        }),
        (Some(subreddit), Some(submission_id), _) => Some(RedditLink::Submission {
            subreddit: subreddit.as_str().into(),
            id: submission_id.as_str().into(),
            media: reddit_post_media(submission_id.as_str()).await,
        }),
        _ => None,
    }
}

async fn reddit_post_media(submission_id: &str) -> Option<Url> {
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

    #[derive(Deserialize, Debug)]
    struct Response {
        data: Data,
    }

    #[derive(Deserialize, Debug)]
    struct Data {
        children: Vec<Child>,
    }

    #[derive(Deserialize, Debug)]
    struct Child {
        data: Data2,
    }

    #[derive(Deserialize, Debug)]
    struct Data2 {
        url: Option<Box<str>>,
    }

    let endpoint_url = format!("https://oauth.reddit.com/comments/{}/", submission_id);
    let response = CLIENT
        .get(endpoint_url)
        .headers(headers)
        .query(&[("api_type", "json")])
        .send()
        .await
        .ok()?
        .json::<Vec<Response>>()
        .await
        .ok()?;

    response
        .first()
        .and_then(|r| r.data.children.first())
        .and_then(|c| c.data.url.clone())
        // Don't follow other reddit links
        .filter(|url| !REDDIT_RE.is_match(url))
        .and_then(|url| Url::parse(&url).ok())
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
