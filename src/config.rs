use serde::Deserialize;
use std::{error::Error, fs};

pub static CONFIG_FILE: &str = "config.toml";

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
