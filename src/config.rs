use once_cell::sync::Lazy;
use serde::Deserialize;
use serenity::utils::Color;
use std::{error::Error, fs};

pub static CONFIG_FILE: &str = "config.toml";
pub static EMBED_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x10, 0x18, 0x20));
pub static EMBED_ERROR_COLOR: Lazy<Color> = Lazy::new(|| Color::from_rgb(0x8a, 0x2a, 0x2b));

#[derive(Debug, Deserialize)]
pub struct Config {
    pub token: String,
    #[serde(default = "default_prefix")]
    pub prefix: String,
}

fn default_prefix() -> String {
    "~".into()
}

impl Config {
    pub fn read_config() -> Result<Self, Box<dyn Error>> {
        let contents = fs::read_to_string(CONFIG_FILE)?;
        Ok(toml::from_str(&contents)?)
    }
}
