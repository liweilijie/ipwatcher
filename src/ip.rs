use anyhow::Result;
use reqwest::Client;
use std::net::IpAddr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpError {
    #[error("No IP sources available")]
    NoSources,
}

/// Query the current external IP by trying a list of sources in order.
/// Returns the first successfully parsed IP.
pub async fn query_external_ip(http: &Client, sources: Option<Vec<String>>) -> Result<IpAddr> {
    let default_sources: Vec<String> = vec![
        "https://api.ipify.org".to_string(),
        "https://ifconfig.me/ip".to_string(),
        "https://ident.me".to_string(),
        "https://checkip.amazonaws.com".to_string(),
    ];
    let list = sources.unwrap_or(default_sources);
    if list.is_empty() {
        return Err(IpError::NoSources.into());
    }

    for url in list {
        match http.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let text = resp.text().await.unwrap_or_default();
                let trimmed = text.trim();
                if let Ok(ip) = trimmed.parse::<IpAddr>() {
                    return Ok(ip);
                }
            }
            _ => {}
        }
    }

    Err(anyhow::anyhow!("All IP sources failed or returned invalid data"))
}
