use std::{fmt::Display, error::Error};

#[derive(Debug)]
pub enum MusicError {
    BadArgument,
    BadIndex,
    BadSource,
    NoPausedTrack,
    NoPlayingTrack,
    NotInVoiceChannel,
    RemoveTrack,
}

impl MusicError {
    pub fn as_str(&self) -> &str {
        match self {
            Self::BadArgument => "Invalid argument",
            Self::BadIndex => "Invalid index",
            Self::BadSource => "Could not load source",
            Self::NoPausedTrack => "No currently paused track",
            Self::NoPlayingTrack => "No currently playing track",
            Self::NotInVoiceChannel => "Not in a voice channel",
            Self::RemoveTrack => "Could not remove track",
        }
    }
}

impl Display for MusicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Error for MusicError {}
