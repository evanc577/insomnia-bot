use anyhow::Result;
use figment::providers::{Env, Format, Toml};
use figment::Figment;
use once_cell::sync::Lazy;
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use serenity::utils::Color;

pub static CONFIG_FILE: &str = "config.toml";
pub static EMBED_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x10, 0x18, 0x20));
pub static EMBED_PLAYING_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x77, 0xDD, 0x77));
pub static EMBED_ERROR_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x8a, 0x2a, 0x2b));

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub discord_token: String,
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
        Ok(Figment::new()
            .merge(Toml::file(CONFIG_FILE))
            .merge(Env::prefixed("INSOMNIA_"))
            .extract()?)
    }
}
