mod dubz;
mod reddit;
mod tweet;

use std::sync::{Arc, LazyLock};

use itertools::Itertools;
use poise::serenity_prelude::prelude::SerenityError;
use poise::serenity_prelude::{EditMessage, Http, Message};
use regex::Regex;
use url::Url;

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

#[derive(Debug)]
enum UrlReplacer {
    Twitter,
    Reddit,
    Dubz,
}

impl UrlReplacer {
    fn dispatch_host(url: &Url) -> Option<Self> {
        // Twitter
        static TWITTER_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(^|\.)(twitter|x)\.com$").unwrap());
        if is_handled_host([&*TWITTER_RE].as_slice(), url) {
            return Some(Self::Twitter);
        }

        // Reddit
        static REDDIT_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(^|\.)reddit\.com$").unwrap());
        static V_REDDIT_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^v.redd\.it$").unwrap());
        if is_handled_host([&*REDDIT_RE, &*V_REDDIT_RE].as_slice(), url) {
            return Some(Self::Reddit);
        }

        // Dubz
        static DUBZ_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(^|\.)dubz\.(co|link)$").unwrap());
        if is_handled_host([&*DUBZ_RE].as_slice(), url) {
            return Some(Self::Dubz);
        }

        None
    }

    async fn replace(&self, url: &Url) -> impl IntoIterator<Item = Url> {
        match self {
            Self::Twitter => Self::replace_tweet(url).await,
            Self::Reddit => Self::replace_reddit(url).await,
            Self::Dubz => Self::replace_dubz(url).await,
        }
    }
}

fn is_handled_host(re: &[&Regex], url: &Url) -> bool {
    re.iter()
        .filter_map(|re| url.host_str().map(|host| re.is_match(host)))
        .any(|m| m)
}

pub async fn reply_link_embeds(
    http: Arc<Http>,
    mut message: Message,
) -> Result<(), LinkEmbedError> {
    // Ignore messages from self
    if message.author.id == http.get_current_user().await.unwrap().id {
        return Ok(());
    }

    #[derive(Debug)]
    enum Node {
        Root,
        Node(Url),
    }

    let arena = &mut indextree::Arena::new();
    let root = arena.new_node(Node::Root);

    // Inital list of urls
    for url in extract_urls(&message.content) {
        let node = arena.new_node(Node::Node(url));
        root.append(node, arena);
    }

    // Recursively replace urls
    for _ in 0..4 {
        let mut new_urls = Vec::new();
        for cur_node_id in root.descendants(arena).skip(1) {
            match arena[cur_node_id].get() {
                Node::Root => unreachable!(),
                Node::Node(url) => {
                    // Skip non-leaf nodes
                    if arena[cur_node_id].first_child().is_some() {
                        continue;
                    }

                    // Get replacement urls
                    if let Some(dispatcher) = UrlReplacer::dispatch_host(url) {
                        new_urls = dispatcher
                            .replace(url)
                            .await
                            .into_iter()
                            .map(|url| (cur_node_id, url))
                            .collect();
                    }
                }
            }
        }

        // Append all replacement urls to current node
        let mut new_urls_processed = false;
        for (cur_node_id, new_url) in new_urls {
            let node_id = arena.new_node(Node::Node(new_url));
            cur_node_id.append(node_id, arena);
            new_urls_processed = true;
        }

        // Finish if no new urls were processed
        if !new_urls_processed {
            break;
        }
    }

    // Gather all replacement urls
    let reply_content = root
        .descendants(arena)
        .skip(1) // Skip root node
        .filter_map(|node| {
            let node = &arena[node];

            // Skip unprocessed urls
            if node.parent() == Some(root) {
                return None;
            }
            // Skip non-leaf nodes
            if node.first_child().is_some() {
                return None;
            }

            if let Node::Node(url) = node.get() {
                return Some(url);
            }

            None
        })
        .map(|url| url.as_str())
        .unique_by(|url| *url)
        .map(|url| url.to_owned())
        .join("\n");

    // Send message
    if !reply_content.is_empty() {
        eprintln!("Replying with updated link in {}", message.channel_id.get());
        message
            .reply(&http, reply_content)
            .await
            .map_err(|e| LinkEmbedError {
                context: "Send message".into(),
                inner: e,
            })?;
        message
            .edit(&http, EditMessage::new().suppress_embeds(true))
            .await
            .map_err(|e| LinkEmbedError {
                context: "Suppress original embeds".into(),
                inner: e,
            })?;
    }
    Ok(())
}

fn extract_urls(text: &str) -> Vec<Url> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"https?://(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*)").unwrap()
    });

    RE.find_iter(text)
        .filter_map(|m| Url::parse(m.as_str()).ok())
        .collect()
}
