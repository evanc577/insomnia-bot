use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
#[allow(dead_code)]
pub enum MusicError {
    BadArgument,
    BadIndex,
    BadPlaylist,
    BadSource,
    GetVoice,
    JoinVoice,
    Loudness,
    NoPausedTrack,
    NoPlayingTrack,
    NotInVoiceChannel,
    QueueLock,
    RemoveTrack,
    VoiceLock,
}

impl Display for MusicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadArgument => write!(f, "Invalid argument"),
            Self::BadIndex => write!(f, "Invalid index"),
            Self::BadPlaylist => write!(f, "Invalid or empty playlist"),
            Self::BadSource => write!(f, "Could not load source"),
            Self::GetVoice => write!(f, "Error: could not get voice channel"),
            Self::JoinVoice => write!(f, "Error: could not join voice channel"),
            Self::Loudness => write!(f, "Error: could not get track loudness"),
            Self::NoPausedTrack => write!(f, "No currently paused track"),
            Self::NoPlayingTrack => write!(f, "No currently playing track"),
            Self::NotInVoiceChannel => write!(f, "Not in a voice channel"),
            Self::QueueLock => write!(f, "Error: could not get queue lock"),
            Self::RemoveTrack => write!(f, "Could not remove track"),
            Self::VoiceLock => write!(f, "Error: could not get voice channel lock"),
        }
    }
}

impl Error for MusicError {}
