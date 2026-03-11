use qimen_error::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::PathBuf;

pub struct PluginStorage {
    base_dir: PathBuf,
}

impl PluginStorage {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn plugin_dir(&self, plugin_id: &str) -> PathBuf {
        self.base_dir.join(plugin_id)
    }

    fn key_path(&self, plugin_id: &str, key: &str) -> PathBuf {
        self.plugin_dir(plugin_id).join(format!("{}.json", key))
    }

    pub async fn get<T: DeserializeOwned>(&self, plugin_id: &str, key: &str) -> Result<Option<T>> {
        let path = self.key_path(plugin_id, key);

        match tokio::fs::read_to_string(&path).await {
            Ok(contents) => {
                let value: T = serde_json::from_str(&contents)?;
                Ok(Some(value))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn set<T: Serialize>(&self, plugin_id: &str, key: &str, value: &T) -> Result<()> {
        let dir = self.plugin_dir(plugin_id);
        tokio::fs::create_dir_all(&dir).await?;

        let path = self.key_path(plugin_id, key);
        let contents = serde_json::to_string_pretty(value)?;
        tokio::fs::write(&path, contents).await?;

        tracing::debug!(plugin_id = %plugin_id, key = %key, "stored value");
        Ok(())
    }

    pub async fn delete(&self, plugin_id: &str, key: &str) -> Result<()> {
        let path = self.key_path(plugin_id, key);

        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                tracing::debug!(plugin_id = %plugin_id, key = %key, "deleted value");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the data directory path for a specific plugin.
    /// Creates the directory if it doesn't exist.
    pub async fn get_data_path(&self, plugin_id: &str) -> Result<std::path::PathBuf> {
        let path = self.base_dir.join(plugin_id);
        if !path.exists() {
            tokio::fs::create_dir_all(&path).await.map_err(|e| {
                qimen_error::QimenError::Runtime(format!(
                    "failed to create plugin data directory '{}': {e}",
                    path.display()
                ))
            })?;
        }
        Ok(path)
    }

    /// Check if a key exists for a plugin
    pub async fn exists(&self, plugin_id: &str, key: &str) -> bool {
        let path = self.base_dir.join(plugin_id).join(format!("{key}.json"));
        path.exists()
    }

    pub async fn list_keys(&self, plugin_id: &str) -> Result<Vec<String>> {
        let dir = self.plugin_dir(plugin_id);

        let mut keys = Vec::new();

        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(keys),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    keys.push(stem.to_string());
                }
            }
        }

        Ok(keys)
    }
}
