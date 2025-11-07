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

    #[serde(default = "default_database_user")]
    pub database_user: String,
    #[serde(default = "default_database_host")]
    pub database_host: String,
    #[serde(default = "default_database_port")]
    pub database_port: u64,
    pub database_password: String,
}

fn default_prefix() -> String {
    "~".into()
}

fn default_database_user() -> String {
    "postgres".into()
}

fn default_database_host() -> String {
    "localhost".into()
}

fn default_database_port() -> u64 {
    5432
}

impl Config {
    pub fn get_config() -> Result<Self> {
        Ok(Figment::new()
            .merge(Toml::file(CONFIG_FILE))
            .merge(Env::prefixed("INSOMNIA_"))
            .extract()?)
    }
}
