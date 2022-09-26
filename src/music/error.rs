use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum MusicError {
    Internal(anyhow::Error),
    AddTracks { failed: usize, total: usize },
    BadIndex,
    BadPlaylist,
    BadSource(String),
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
            Self::AddTracks { failed, total } => {
                if *total == 1 {
                    write!(f, "could not add track to queue")
                } else {
                    write!(f, "could not add {} tracks to queue", failed)
                }
            }
            Self::BadIndex => write!(f, "invalid index"),
            Self::BadPlaylist => write!(f, "invalid or empty playlist"),
            Self::BadSource(s) => {
                let s = s
                    .trim_start_matches("ERROR:")
                    .trim_start_matches("Error:")
                    .trim_start_matches("error:")
                    .trim();
                write!(f, "could not load source\n{}", s)
            }
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
