use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
#[allow(dead_code)]
pub enum MusicError {
    BadArgument,
    BadIndex,
    BadPlaylist,
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
            Self::BadPlaylist => "Invalid or empty playlist",
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
