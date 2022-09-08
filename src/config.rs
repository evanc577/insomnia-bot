use std::{env, fs};

use anyhow::Result;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serenity::utils::Color;

use crate::error::InsomniaError;

pub static CONFIG_FILE: &str = "config.toml";
pub static EMBED_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x10, 0x18, 0x20));
pub static EMBED_ERROR_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x8a, 0x2a, 0x2b));

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub token: String,
    #[serde(default = "default_prefix")]
    pub prefix: String,
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
            },
        };

        if let Ok(token) = env::var("DISCORD_TOKEN") {
            config.token = token;
        }
        if let Ok(prefix) = env::var("DISCORD_COMMAND_PREFIX") {
            config.prefix = prefix;
        }

        if config.token.is_empty() {
            Err(InsomniaError::ConfigToken.into())
        } else {
            Ok(config)
        }
    }

    fn read_config() -> Result<Self> {
        let contents = fs::read_to_string(CONFIG_FILE)?;
        Ok(toml::from_str(&contents)?)
    }
}
