use std::path::PathBuf;

use anyhow::Result;
use iroh::SecretKey;
use log::{info, warn};

use crate::{Node, config::Config};

// Repo builds up a series of file path conventions from a root directory path.
pub struct Repo(PathBuf);

impl Repo {
    const KEY_FILE: &str = "key";
    const CONFIG_FILE: &str = "config.yml";

    pub fn default_location() -> PathBuf {
        dirs_next::data_local_dir().unwrap().join("datum_agent")
    }

    /// Opens or creates a repo at the given base directory.
    pub async fn open_or_create(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let base_dir = base_dir.into();
        tokio::fs::create_dir_all(&base_dir).await?;
        info!("opening repo at {}", base_dir.display());

        let this = Self(base_dir);

        Ok(this)
    }

    pub async fn spawn_node(&self) -> Result<Node> {
        let cfg = self.config().await?;
        let secret = self.secret_key().await?;
        let node = Node::new(secret, &cfg).await.unwrap();
        Ok(node)
    }

    /// reads the config file as a string
    pub async fn config(&self) -> Result<Config> {
        let config_file_path = self.0.join(Self::CONFIG_FILE);
        if !config_file_path.exists() {
            warn!("secret key does not exist. creating new key");
            let cfg = Config::default();
            cfg.write(config_file_path).await?;
            return Ok(cfg);
        };

        Config::from_file(config_file_path).await
    }

    pub async fn secret_key(&self) -> Result<SecretKey> {
        let key_file_path = self.0.join(Self::KEY_FILE);
        if !key_file_path.exists() {
            warn!("secret key does not exist. creating new key");
            tokio::fs::create_dir_all(&self.0).await?;
            return self.create_key().await;
        };

        let key = tokio::fs::read(key_file_path).await?;
        let key = key.as_slice().try_into()?;
        Ok(SecretKey::from_bytes(key))
    }

    async fn create_key(&self) -> Result<SecretKey> {
        let key_file_path = self.0.join(Self::KEY_FILE);
        let key = SecretKey::generate(&mut rand::rng());
        tokio::fs::write(key_file_path, key.to_bytes()).await?;
        Ok(key)
    }
}
