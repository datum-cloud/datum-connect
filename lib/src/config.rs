use std::{
    fs,
    net::{SocketAddrV4, SocketAddrV6},
    path::PathBuf,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// The IPv4 address that the endpoint will listen on.
    ///
    /// If None, defaults to a random free port, but it can be useful to specify a fixed
    /// port, e.g. to configure a firewall rule.
    pub ipv4_addr: Option<SocketAddrV4>,

    /// The IPv6 address that the endpoint will listen on.
    ///
    /// If None, defaults to a random free port, but it can be useful to specify a fixed
    /// port, e.g. to configure a firewall rule.
    pub ipv6_addr: Option<SocketAddrV6>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ipv4_addr: None,
            ipv6_addr: None,
        }
    }
}

impl Config {
    pub async fn from_file(path: PathBuf) -> Result<Self> {
        let config = tokio::fs::read_to_string(path)
            .await
            .context("reading config file")?;
        let config = toml::from_str(&config).context("paring config file")?;
        Ok(config)
    }

    pub async fn write(self, path: PathBuf) -> Result<Self> {
        fs::write(path, toml::to_string_pretty(&self)?)?;

        Ok(self)
    }
}
