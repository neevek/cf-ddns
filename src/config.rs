use anyhow::{Result, bail};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub api_token: String,
    pub zone: String,
    #[serde(default)]
    pub record_name: Option<String>,
    #[serde(default = "default_record_type")]
    pub record_type: String,
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
    #[serde(default)]
    pub interface_name: Option<String>,
    #[serde(default)]
    pub proxied: Option<bool>,
    #[serde(default)]
    pub ttl: Option<u32>,
}

fn default_record_type() -> String {
    "A".to_string()
}

fn default_interval_seconds() -> u64 {
    300
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&raw)?;
        config.validate()?;
        Ok(config)
    }

    pub fn record_name(&self) -> String {
        self.record_name
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.zone.clone())
    }

    pub fn validate(&self) -> Result<()> {
        if self.api_token.trim().is_empty() {
            bail!("api_token must not be empty");
        }
        if self.zone.trim().is_empty() {
            bail!("zone must not be empty");
        }
        if self.interval_seconds == 0 {
            bail!("interval_seconds must be greater than 0");
        }
        Ok(())
    }
}
