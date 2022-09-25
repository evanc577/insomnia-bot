use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum MusicError {
    Internal(anyhow::Error),
    BadIndex,
    BadPlaylist,
    BadSource,
    GetVoice,
    JoinVoice,
    Loudness,
    NoPausedTrack,
    NoPlayingTrack,
    NoResults,
    NotInVoiceChannel,
    RemoveTrack,
}

impl Display for MusicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal(e) => write!(f, "internal error: {}", e),
            Self::BadIndex => write!(f, "invalid index"),
            Self::BadPlaylist => write!(f, "invalid or empty playlist"),
            Self::BadSource => write!(f, "could not load source"),
            Self::GetVoice => write!(f, "could not get voice channel"),
            Self::JoinVoice => write!(f, "could not join voice channel"),
            Self::Loudness => write!(f, "could not get track loudness"),
            Self::NoPausedTrack => write!(f, "no currently paused track"),
            Self::NoPlayingTrack => write!(f, "no currently playing track"),
            Self::NoResults => write!(f, "no results found"),
            Self::NotInVoiceChannel => write!(f, "you are not in a voice channel"),
            Self::RemoveTrack => write!(f, "could not remove track"),
        }
    }
}

impl Error for MusicError {}

#[derive(Debug)]
pub enum InternalError {
    QueueLock,
}

impl Display for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueLock => write!(f, "could not get queue lock"),
        }
    }
}

impl Error for InternalError {}
