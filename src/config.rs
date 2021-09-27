use once_cell::sync::Lazy;
use serde::Deserialize;
use serenity::utils::Color;
use std::{env, error::Error, fmt::Display, fs};

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

#[derive(Debug)]
struct ConfigError {}

impl Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Config error")
    }
}

impl Error for ConfigError {}

impl Config {
    pub fn get_config() -> Result<Self, Box<dyn Error>> {
        let mut config = match Self::read_config() {
            Ok(c) => c,
            Err(_) => Config {
                token: "".into(),
                prefix: default_prefix(),
            }
        };

        if let Ok(token) = env::var("DISCORD_TOKEN") {
            config.token = token;
        }
        if let Ok(prefix) = env::var("DISCORD_COMMAND_PREFIX") {
            config.prefix = prefix;
        }

        if config.token == "" {
            Err(ConfigError {})?
        } else {
            Ok(config)
        }
    }

    fn read_config() -> Result<Self, Box<dyn Error>> {
        let contents = fs::read_to_string(CONFIG_FILE)?;
        Ok(toml::from_str(&contents)?)
    }
}
