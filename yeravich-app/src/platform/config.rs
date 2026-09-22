use std::{path::PathBuf, sync::Arc};

use yeravich_core::{
    AppConfig, CoreFuture,
    settings::{ConfigStore, ConfigStoreError},
};

use super::storage;

pub struct FileConfigStore {
    path: Arc<PathBuf>,
}

impl FileConfigStore {
    pub fn new() -> Result<Self, ConfigStoreError> {
        Ok(Self {
            path: Arc::new(storage::directory()?.join("config.toml")),
        })
    }
}

impl ConfigStore for FileConfigStore {
    fn load(&self) -> CoreFuture<Result<AppConfig, ConfigStoreError>> {
        let path = Arc::clone(&self.path);
        Box::pin(async move {
            match tokio::fs::read_to_string(path.as_ref()).await {
                Ok(contents) => {
                    AppConfig::from_toml(&contents).map_err(|_| ConfigStoreError::Invalid)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(AppConfig::default())
                }
                Err(_) => Err(ConfigStoreError::Read),
            }
        })
    }

    fn save(&self, config: AppConfig) -> CoreFuture<Result<(), ConfigStoreError>> {
        let path = Arc::clone(&self.path);
        Box::pin(async move {
            let contents = config.to_toml().map_err(|_| ConfigStoreError::Write)?;
            storage::atomic_write(&path, contents.as_bytes())
                .await
                .map_err(|_| ConfigStoreError::Write)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn persists_config_and_rejects_corruption_without_overwriting_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = Arc::new(directory.path().join("config.toml"));
        let store = FileConfigStore {
            path: Arc::clone(&path),
        };
        assert_eq!(store.load().await.unwrap(), AppConfig::default());
        let mut config = AppConfig::default();
        config.swap_languages();
        store.save(config.clone()).await.unwrap();
        assert_eq!(store.load().await.unwrap(), config);
        tokio::fs::write(path.as_ref(), "malformed TOML")
            .await
            .unwrap();
        assert_eq!(store.load().await.unwrap_err(), ConfigStoreError::Invalid);
        assert_eq!(
            tokio::fs::read_to_string(path.as_ref()).await.unwrap(),
            "malformed TOML"
        );
    }
}
