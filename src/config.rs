use anyhow::Result;
use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

pub static CONFIG_FILE: &str = "config.toml";

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
