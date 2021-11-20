use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum InsomniaError {
    ConfigToken,
    JoinVoice,
    GetVoice,
    VoiceLock,
    QueueLock,
    Loudness,
}

impl Display for InsomniaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigToken => write!(f, "Error: no Discord token"),
            Self::JoinVoice => write!(f, "Error: could not join voice channel"),
            Self::GetVoice => write!(f, "Error: could not get voice channel"),
            Self::VoiceLock => write!(f, "Error: could not get voice channel lock"),
            Self::QueueLock => write!(f, "Error: could not get queue lock"),
            Self::Loudness => write!(f, "Error: could not get track loudness"),
        }
    }
}

impl Error for InsomniaError {}
