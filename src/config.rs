use anyhow::{bail, Result};
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
    #[serde(default = "default_retry_seconds")]
    pub retry_seconds: u64,
    #[serde(default)]
    pub interface_name: Option<String>,
    #[serde(default)]
    pub proxied: Option<bool>,
    #[serde(default)]
    pub ttl: Option<u32>,
    #[serde(default)]
    pub use_public_ip: bool,
    #[serde(default = "default_public_ipv4_urls")]
    pub public_ipv4_urls: Vec<String>,
    #[serde(default = "default_public_ipv6_urls")]
    pub public_ipv6_urls: Vec<String>,
}

fn default_record_type() -> String {
    "A".to_string()
}

fn default_interval_seconds() -> u64 {
    300
}

fn default_retry_seconds() -> u64 {
    30
}

fn default_public_ipv4_urls() -> Vec<String> {
    vec![
        "https://api.ipify.org".to_string(),
        "https://ifconfig.me/ip".to_string(),
        "https://icanhazip.com".to_string(),
    ]
}

fn default_public_ipv6_urls() -> Vec<String> {
    vec![
        "https://api64.ipify.org".to_string(),
        "https://ipv6.icanhazip.com".to_string(),
        "https://ifconfig.co/ip".to_string(),
    ]
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
        if self.retry_seconds == 0 {
            bail!("retry_seconds must be greater than 0");
        }
        if self.use_public_ip {
            let record_type = self.record_type.trim().to_ascii_uppercase();
            match record_type.as_str() {
                "A" => {
                    if self.public_ipv4_urls.is_empty() {
                        bail!("public_ipv4_urls must not be empty when use_public_ip is true");
                    }
                }
                "AAAA" => {
                    if self.public_ipv6_urls.is_empty() {
                        bail!("public_ipv6_urls must not be empty when use_public_ip is true");
                    }
                }
                _ => {
                    bail!("record_type must be A or AAAA when use_public_ip is true");
                }
            }
        }
        Ok(())
    }
}
