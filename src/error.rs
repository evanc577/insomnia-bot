use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum InsomniaError {
    ConfigToken,
    JoinVoice,
    GetVoice,
}

impl Display for InsomniaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigToken => write!(f, "Error: no Discord token"),
            Self::JoinVoice => write!(f, "Error: can't join voice channel"),
            Self::GetVoice => write!(f, "Error: can't get voice channel"),
        }
    }
}

impl Error for InsomniaError {}
