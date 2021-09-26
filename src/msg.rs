use std::fmt::Display;

use markdown::{generate_markdown, Block, Span};
use once_cell::sync::Lazy;
use serenity::{builder::CreateEmbed, http::Http, model::id::ChannelId};
use songbird::tracks::TrackHandle;

use crate::config::{EMBED_COLOR, EMBED_ERROR_COLOR};

pub fn format_track(track: &TrackHandle, format: bool) -> String {
    let artist = track
        .metadata()
        .artist
        .as_ref()
        .unwrap_or(&"Unknown".to_owned())
        .to_owned();
    let title = track
        .metadata()
        .title
        .as_ref()
        .unwrap_or(&"Unknown".to_owned())
        .to_owned();

    let raw = format!("{} - {}", artist, title);

    if format {
        format!(
            "**{}**",
            raw.replace("*", "\\*")
                .replace("_", "\\_")
                .replace("~", "\\~")
                .replace("`", "")
        )
    } else {
        raw
    }
}

pub async fn send_error_embed(http: &Http, channel_id: ChannelId, message: &str) {
    let _ = channel_id
        .send_message(http, |m| {
            m.embed(|e| {
                e.title("Error");
                e.description(message);
                e.color(*EMBED_ERROR_COLOR);
                e
            })
        })
        .await;
}

pub async fn send_embed(http: &Http, channel_id: ChannelId, message: &str) {
    let _ = channel_id
        .send_message(http, |m| {
            m.embed(|e| {
                e.description(message);
                e
            })
        })
        .await;
}

#[allow(dead_code)]
pub enum PlayUpdate {
    Add(usize),
    Play,
    Pause,
    Resume,
    Skip,
    Remove,
}

impl PlayUpdate {
    fn detailed(&self) -> bool {
        match self {
            Self::Add(_) => true,
            Self::Play => true,
            _ => false,
        }
    }

    fn queue_size(&self) -> Option<usize> {
        match self {
            Self::Add(n) => Some(*n),
            Self::Play => Some(1),
            _ => None,
        }
    }
}

impl Display for PlayUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match *self {
            Self::Add(_) => "Queued",
            Self::Play => "Playing",
            Self::Pause => "Paused",
            Self::Resume => "Resumed",
            Self::Skip => "Skipped",
            Self::Remove => "Removed",
        };
        write!(f, "{}", text)
    }
}

static SEP_SPAN: Lazy<Span> = Lazy::new(|| Span::Text(" ".into()));

pub async fn send_playback_update_embed(
    http: &Http,
    channel_id: ChannelId,
    track: &TrackHandle,
    update: PlayUpdate,
) {
    // Generate description
    let update_span = Span::Text(update.to_string());
    let track_span = Span::Strong(vec![format_track_link(track)]);
    let description_block = Block::Paragraph(vec![update_span, SEP_SPAN.clone(), track_span]);
    let description_text = generate_markdown(vec![description_block]);

    let _ = channel_id
        .send_message(http, |m| {
            m.embed(|e| {
                e.description(description_text);
                if update.detailed() {
                    add_details(e, track, update);
                }
                e.color(*EMBED_COLOR);
                e
            })
        })
        .await;
}

fn add_details(embed: &mut CreateEmbed, track: &TrackHandle, update: PlayUpdate) {
    static ARTIST_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Artist".into()));
    static DURATION_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Length".into()));
    static QUEUE_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Queue".into()));

    let mut fields = vec![];

    // Artist
    let artist_name_text =
        generate_markdown(vec![Block::Paragraph(vec![ARTIST_NAME_SPAN.clone()])]);
    let artist_value_text =
        generate_markdown(vec![Block::Paragraph(vec![format_artist(track)])]);
    fields.push((artist_name_text, artist_value_text, true));

    // Duration
    let duration_name_text =
        generate_markdown(vec![Block::Paragraph(vec![DURATION_NAME_SPAN.clone()])]);
    let duration_value_text =
        generate_markdown(vec![Block::Paragraph(vec![format_duration(track)])]);
    fields.push((duration_name_text, duration_value_text, true));

    // Thumbnail
    if let Some(url) = &track.metadata().thumbnail {
        embed.thumbnail(url);
    }

    // Queue size
    if let Some(n) = update.queue_size() {
        let queue_name_text =
            generate_markdown(vec![Block::Paragraph(vec![QUEUE_NAME_SPAN.clone()])]);
        let queue_value_text =
            generate_markdown(vec![Block::Paragraph(vec![Span::Text(n.to_string())])]);
        fields.push((queue_name_text, queue_value_text, true));
    }

    // Add details to embed
    if !fields.is_empty() {
        embed.fields(fields);
    }
}

fn format_track_link(track: &TrackHandle) -> Span {
    let title = track
        .metadata()
        .title
        .clone()
        .unwrap_or("Unknown title".into());
    let span = match track.metadata().source_url.clone() {
        Some(u) => Span::Link(title, u, None),
        None => Span::Text(title),
    };
    span
}

fn format_artist(track: &TrackHandle) -> Span {
    let artist = track
        .metadata()
        .artist
        .clone()
        .unwrap_or("Unknown title".into());
    Span::Text(artist)
}

fn format_duration(track: &TrackHandle) -> Span {
    let duration = match track.metadata().duration {
        Some(d) => d,
        None => return Span::Text("Unknown".into()),
    };

    // compute hours mins secs
    let total_secs = duration.as_secs();
    let secs = total_secs % 60;
    let mins = total_secs / 60 % 60;
    let hours = total_secs / 60 / 60;

    // compute strings
    let mut ret = "".to_owned();
    if hours != 0 {
        ret.push_str(&format!("{}:{:02}:{:02}", hours, mins, secs));
    } else {
        ret.push_str(&format!("{}:{:02}", mins, secs));
    }

    Span::Text(ret)
}
