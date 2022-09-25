use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
#[allow(dead_code)]
pub enum InsomniaError {
    ConfigToken,
    SpotifySecret,
}

impl Display for InsomniaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigToken => write!(f, "Error: no Discord token"),
            Self::SpotifySecret => write!(f, "Error: no SpotifySecret"),
        }
    }
}

impl Error for InsomniaError {}
