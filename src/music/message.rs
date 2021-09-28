use std::fmt::Display;

use markdown::{generate_markdown, Block, Span};
use once_cell::sync::Lazy;
use serenity::builder::CreateEmbed;
use songbird::tracks::TrackHandle;

use crate::config::EMBED_COLOR;


#[derive(Clone, Copy, PartialEq)]
pub enum PlayUpdate {
    Add(usize),
    Play(usize),
    Pause,
    Resume,
    Skip,
    Remove,
}

impl PlayUpdate {
    fn detailed(&self) -> bool {
        matches!(self, Self::Play(_))
    }

    fn queue_size(&self) -> Option<usize> {
        match self {
            Self::Add(n) => Some(*n),
            Self::Play(n) => Some(*n),
            _ => None,
        }
    }
}

impl Display for PlayUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match *self {
            Self::Add(_) => "Queued",
            Self::Play(_) => "Playing",
            Self::Pause => "Paused",
            Self::Resume => "Resumed",
            Self::Skip => "Skipped",
            Self::Remove => "Removed",
        };
        write!(f, "{}", text)
    }
}

pub fn format_update(
    track: &TrackHandle,
    update: PlayUpdate,
) -> Box<dyn FnOnce(&mut CreateEmbed) + Send + '_> {
    Box::new(move |e| {
        // Generate title
        let title_span = Span::Text(update.to_string());
        let title_block = Block::Paragraph(vec![title_span]);
        let title_text = generate_markdown(vec![title_block]);

        // Generate description
        let description_span = Span::Strong(vec![format_track_link(track)]);
        let description_block = Block::Paragraph(vec![description_span]);
        let description_text = generate_markdown(vec![description_block]);

        e.title(title_text);
        e.description(description_text);
        if update.detailed() {
            add_details(e, track, update);
        }
        e.color(*EMBED_COLOR);
    })
}

fn add_details(embed: &mut CreateEmbed, track: &TrackHandle, update: PlayUpdate) {
    static ARTIST_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Artist".into()));
    static DURATION_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Length".into()));
    static QUEUE_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Queue".into()));

    let mut fields = vec![];

    // Artist
    let artist_name_text =
        generate_markdown(vec![Block::Paragraph(vec![ARTIST_NAME_SPAN.clone()])]);
    let artist_value_text = generate_markdown(vec![Block::Paragraph(vec![format_artist(track)])]);
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
    let title = track.metadata().title.clone().unwrap_or_else(|| "Unknown".into());
    let span = match track.metadata().source_url.clone() {
        Some(u) => Span::Link(title, u, None),
        None => Span::Text(title),
    };
    span
}

fn format_artist(track: &TrackHandle) -> Span {
    let artist = track.metadata().artist.clone().unwrap_or_else(|| "Unknown".into());
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