use std::{env, fs};

use anyhow::Result;
use once_cell::sync::Lazy;
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use serenity::utils::Color;

use crate::error::InsomniaError;

pub static CONFIG_FILE: &str = "config.toml";
pub static EMBED_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x10, 0x18, 0x20));
pub static EMBED_PLAYING_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x77, 0xDD, 0x77));
pub static EMBED_ERROR_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x8a, 0x2a, 0x2b));

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub token: String,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    pub spotify_client_id: String,
    pub spotify_secret: String,
}

fn default_prefix() -> String {
    "~".into()
}

impl Config {
    pub fn get_config() -> Result<Self> {
        let mut config = match Self::read_config() {
            Ok(c) => c,
            Err(_) => Config {
                token: "".into(),
                prefix: default_prefix(),
                spotify_client_id: "".into(),
                spotify_secret: "".into(),
            },
        };

        if let Ok(token) = env::var("DISCORD_TOKEN") {
            config.token = token;
        }
        if let Ok(prefix) = env::var("DISCORD_COMMAND_PREFIX") {
            config.prefix = prefix;
        }
        if let Ok(spotify_client_id) = env::var("SPOTIFY_CLIENT_ID") {
            config.spotify_client_id = spotify_client_id;
        }
        if let Ok(spotify_secret) = env::var("SPOTIFY_SECRET") {
            config.spotify_secret = spotify_secret;
        }

        if config.token.is_empty() {
            Err(InsomniaError::ConfigToken.into())
        } else if config.spotify_secret.is_empty() || config.spotify_client_id.is_empty() {
            Err(InsomniaError::SpotifySecret.into())
        } else {
            Ok(config)
        }
    }

    fn read_config() -> Result<Self> {
        let contents = fs::read_to_string(CONFIG_FILE)?;
        Ok(toml::from_str(&contents)?)
    }
}
