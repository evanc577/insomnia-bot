use std::fmt::Write as _;
use std::time::Duration;

use markdown::{generate_markdown, Block, Span};
use once_cell::sync::Lazy;
use poise::serenity_prelude as serenity;
use serenity::builder::CreateEmbed;
use serenity::Color;
use songbird::tracks::TrackHandle;

use super::youtube::sponsorblock::SBDuration;
use crate::message::{EMBED_COLOR, EMBED_PLAYING_COLOR};

#[derive(Clone)]
pub enum PlayUpdate {
    Add(TrackHandle, usize),
    Play(TrackHandle, usize),
    Pause(TrackHandle),
    Resume(TrackHandle),
    Skip(TrackHandle),
    Remove(TrackHandle),
    Stop,
}

impl PlayUpdate {
    /// Returns a closure that formats a Discord message embed with the PlayUpdate info
    pub async fn format<'a>(&'a self) -> Box<dyn FnOnce(&mut CreateEmbed) + Send + Sync + 'a> {
        // Extract the sponsorblock duration first because we can't call async functions in the
        // returned closure
        let sb_duration = self.sb_duration().await;

        Box::new(move |e| {
            // Embed color
            e.color(self.color());

            // Generate title
            let title_span = Span::Text(self.title().to_string());
            let title_block = Block::Paragraph(vec![title_span]);
            let title_text = generate_markdown(vec![title_block]);
            e.title(title_text);

            // Track info
            if let Some(track) = self.track_handle() {
                // Generate description
                let description_block = if self.detailed() {
                    format_detailed_track_link(&track)
                } else {
                    format_track_link(&track)
                };
                let description_text = generate_markdown(vec![description_block]);

                e.description(description_text);
                if self.detailed() {
                    self.add_details(e, sb_duration);
                }

                // Thumbnail
                if let Some(url) = &track.metadata().thumbnail {
                    e.thumbnail(url);
                }
            }
        })
    }

    /// Title to be displayed in Discord message embed
    fn title(&self) -> &str {
        match self {
            Self::Add(_, _) => "Queued",
            Self::Play(_, _) => "Playing",
            Self::Pause(_) => "Paused",
            Self::Resume(_) => "Resumed",
            Self::Skip(_) => "Skipped",
            Self::Remove(_) => "Removed",
            Self::Stop => "Stopped",
        }
    }

    /// Show extended track information
    fn detailed(&self) -> bool {
        matches!(self, Self::Play(_, _))
    }

    /// Add extended track information to Discord message embed
    fn add_details(&self, embed: &mut CreateEmbed, sb_duration: Option<Duration>) {
        static ARTIST_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Artist".into()));
        static DURATION_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Length".into()));
        static QUEUE_NAME_SPAN: Lazy<Span> = Lazy::new(|| Span::Text("Queue".into()));

        let mut fields = vec![];

        // Track info
        if let Some(track) = self.track_handle() {
            // Artist
            let artist_name_text =
                generate_markdown(vec![Block::Paragraph(vec![ARTIST_NAME_SPAN.clone()])]);
            let artist_value_text =
                generate_markdown(vec![Block::Paragraph(vec![format_artist(&track)])]);
            fields.push((artist_name_text, artist_value_text, true));

            // Duration
            let duration_name_text =
                generate_markdown(vec![Block::Paragraph(vec![DURATION_NAME_SPAN.clone()])]);
            let duration_value_text =
                generate_markdown(vec![Block::Paragraph(vec![format_track_duration(
                    &track,
                    sb_duration,
                )])]);
            fields.push((duration_name_text, duration_value_text, true));
        }

        // Queue size
        if let Some(n) = self.queue_size() {
            if n >= 2 {
                let queue_name_text =
                    generate_markdown(vec![Block::Paragraph(vec![QUEUE_NAME_SPAN.clone()])]);
                let queue_value_text = generate_markdown(vec![Block::Paragraph(vec![Span::Text(
                    (n - 1).to_string(),
                )])]);
                fields.push((queue_name_text, queue_value_text, true));
            }
        }

        // Add details to embed
        if !fields.is_empty() {
            embed.fields(fields);
        }
    }

    /// Current size of queue
    fn queue_size(&self) -> Option<usize> {
        match self {
            Self::Add(_, n) => Some(*n),
            Self::Play(_, n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the track handle associated with this update if available
    fn track_handle(&self) -> Option<TrackHandle> {
        match self {
            Self::Add(t, _) => Some(t.clone()),
            Self::Play(t, _) => Some(t.clone()),
            Self::Pause(t) => Some(t.clone()),
            Self::Resume(t) => Some(t.clone()),
            Self::Skip(t) => Some(t.clone()),
            Self::Remove(t) => Some(t.clone()),
            _ => None,
        }
    }

    /// If sponsorblock segments exist on the track, return the duration with those segments
    /// removed
    async fn sb_duration(&self) -> Option<Duration> {
        if let Some(track_handle) = self.track_handle() {
            if let Some(Some(sb_duration)) = track_handle.typemap().read().await.get::<SBDuration>()
            {
                return Some(*sb_duration);
            }
        }
        None
    }

    /// Color of Discord message embed
    fn color(&self) -> Color {
        match self {
            Self::Play(..) => *EMBED_PLAYING_COLOR,
            _ => *EMBED_COLOR,
        }
    }
}

/// Formats a Discord message embed when adding multiple tracks at once
pub fn format_add_playlist<'a>(
    tracks: impl ExactSizeIterator<Item = TrackHandle> + Send + Sync + 'a,
    num_queued_tracks: usize,
    total_tracks: usize,
    finished: bool,
) -> Box<dyn FnOnce(&mut CreateEmbed) + Send + Sync + 'a> {
    Box::new(move |e| {
        let title_text = if finished {
            format!(
                "Finished Queuing {}/{} tracks",
                num_queued_tracks, total_tracks
            )
        } else {
            format!("Queuing {}/{} tracks", num_queued_tracks, total_tracks)
        };
        let title_span = Span::Text(title_text);
        let title_block = Block::Paragraph(vec![title_span]);
        let title_text = generate_markdown(vec![title_block]);
        e.title(title_text);

        let mut description = Vec::with_capacity(tracks.len() + 1);
        if num_queued_tracks > tracks.len() {
            description.push(Block::Paragraph(vec![Span::Emphasis(vec![Span::Text(
                format!("{} tracks omitted", num_queued_tracks - tracks.len()),
            )])]));
        }

        description.extend(tracks.map(|t| format_track_link(&t)));
        e.description(generate_markdown(description));
    })
}

/// Returns "artist — title"
fn format_track_link(track: &TrackHandle) -> Block {
    let title = track
        .metadata()
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let artist = track
        .metadata()
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let title_span = match track.metadata().source_url.clone() {
        Some(u) => Span::Link(title, u, None),
        None => Span::Text(title),
    };
    let title_span = Span::Strong(vec![title_span]);
    Block::Paragraph(vec![
        Span::Text(artist),
        Span::Text(" — ".to_string()),
        title_span,
    ])
}

/// Only returns the track name, artist and other info will be placed in separate fields
fn format_detailed_track_link(track: &TrackHandle) -> Block {
    let title = track
        .metadata()
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let title_span = match track.metadata().source_url.clone() {
        Some(u) => Span::Link(title, u, None),
        None => Span::Text(title),
    };
    let title_span = Span::Strong(vec![title_span]);
    Block::Paragraph(vec![title_span])
}

fn format_artist(track: &TrackHandle) -> Span {
    let artist = track
        .metadata()
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    Span::Text(artist)
}

fn format_track_duration(track: &TrackHandle, sb_time: Option<Duration>) -> Span {
    let duration = match track.metadata().duration {
        Some(d) => d,
        None => return Span::Text("Unknown".into()),
    };

    let mut ret = format_duration(duration);
    if let Some(t) = sb_time {
        let _ = write!(ret, " ({})", format_duration(duration - t));
    }

    Span::Text(ret)
}

fn format_duration(t: Duration) -> String {
    // compute hours mins secs
    let total_secs = t.as_secs();
    let secs = total_secs % 60;
    let mins = total_secs / 60 % 60;
    let hours = total_secs / 60 / 60;

    // compute strings
    let mut ret = "".to_owned();
    if hours != 0 {
        let _ = write!(ret, "{}:{:02}:{:02}", hours, mins, secs);
    } else {
        let _ = write!(ret, "{}:{:02}", mins, secs);
    }

    ret
}
