use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum PatchbotForwardError {
    InvalidEmbeds,
}

impl Display for PatchbotForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEmbeds => write!(f, "no embeds or invalid embeds"),
        }
    }
}

impl Error for PatchbotForwardError {}
