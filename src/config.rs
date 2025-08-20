use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub check_interval_secs: u64,
    pub db_path: String,
    #[serde(default)]
    pub ip_sources: Option<Vec<String>>,
    pub smtp: SmtpConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmtpConfig {
    pub username: String,
    pub app_password: String,
    pub from: String,
    pub to: String,
    #[serde(default = "default_server")]
    pub server: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_server() -> String { "smtp.gmail.com".to_string() }
fn default_port() -> u16 { 587 }

/// Load config from a TOML file path.
pub fn load_from(path: &str) -> Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {path}"))?;
    let cfg: Config = toml::from_str(&text).context("Failed to parse TOML config")?;
    Ok(cfg)
}
