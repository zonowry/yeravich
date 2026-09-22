//! Directly callable provider settings, persistence, and connection checks.

use std::sync::Arc;

use futures_util::lock::Mutex;
use thiserror::Error;

use crate::{
    AppConfig, CoreFuture, SecretReference, SecretStoreError, SecretValue, Yeravich,
    openai::{ADAPTER_ID, ChatClient, ChatError, ChatSettings, ConnectionReport},
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigStoreError {
    #[error("The configuration directory is unavailable.")]
    Unavailable,
    #[error("Could not read the configuration file.")]
    Read,
    #[error("The configuration file is invalid or uses an unsupported version.")]
    Invalid,
    #[error("Could not save the configuration file.")]
    Write,
}

pub trait ConfigStore: Send + Sync {
    fn load(&self) -> CoreFuture<Result<AppConfig, ConfigStoreError>>;
    fn save(&self, config: AppConfig) -> CoreFuture<Result<(), ConfigStoreError>>;
}

#[derive(Debug)]
pub enum SecretChange {
    Keep,
    Replace(SecretValue),
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SettingsError {
    #[error("{0}")]
    Chat(#[from] ChatError),
    #[error("{0}")]
    Config(#[from] ConfigStoreError),
    #[error("Could not access the saved API key: {0}")]
    Secret(#[from] SecretStoreError),
}

#[derive(Clone)]
pub struct SettingsService {
    core: Yeravich,
    store: Arc<dyn ConfigStore>,
    client: ChatClient,
    writes: Arc<Mutex<()>>,
}

impl SettingsService {
    #[must_use]
    pub fn new(core: Yeravich, store: Arc<dyn ConfigStore>, client: ChatClient) -> Self {
        Self {
            core,
            store,
            client,
            writes: Arc::default(),
        }
    }

    /// Loads saved settings before allowing edits or translation.
    ///
    /// # Errors
    /// Returns a sanitized storage/schema error without overwriting invalid files.
    pub async fn load(&self) -> Result<(), SettingsError> {
        self.core.replace_config(self.store.load().await?);
        Ok(())
    }

    #[must_use]
    pub fn saved_profile(&self) -> Option<crate::ProviderProfile> {
        self.core
            .config()
            .providers
            .into_iter()
            .find(|profile| profile.name == ADAPTER_ID)
    }

    async fn key_for_test(
        &self,
        change: SecretChange,
    ) -> Result<Option<SecretValue>, SettingsError> {
        match change {
            SecretChange::Replace(secret) => Ok(Some(secret)),
            SecretChange::Remove => Ok(None),
            SecretChange::Keep => {
                if let Some(reference) = self.retained_key() {
                    Ok(Some(self.core.secrets().load(&reference).await?))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn retained_key(&self) -> Option<SecretReference> {
        self.saved_profile().and_then(|profile| profile.secret_ref)
    }

    /// Tests the draft without saving it or exposing the existing key to the UI.
    ///
    /// # Errors
    /// Returns validation, credential-store, or sanitized HTTP errors.
    pub async fn test_connection(
        &self,
        settings: ChatSettings,
        change: SecretChange,
    ) -> Result<ConnectionReport, SettingsError> {
        let secret = self.key_for_test(change).await?;
        Ok(self.client.test_connection(&settings, secret).await?)
    }

    /// Saves a new credential before atomically committing its opaque reference.
    /// The active in-memory configuration changes only after persistence succeeds.
    ///
    /// # Errors
    /// Returns validation or storage errors, leaving the previous active profile intact.
    pub async fn save(
        &self,
        settings: ChatSettings,
        change: SecretChange,
    ) -> Result<(), SettingsError> {
        let _guard = self.writes.lock().await;
        let previous = self.retained_key();
        let (reference, created) = match change {
            SecretChange::Keep => (self.retained_key(), false),
            SecretChange::Remove => (None, false),
            SecretChange::Replace(secret) => {
                let reference = SecretReference(format!("provider:{}", uuid::Uuid::new_v4()));
                self.core.secrets().save(&reference, secret).await?;
                (Some(reference), true)
            }
        };
        let mut config = self.core.config();
        config
            .providers
            .retain(|profile| profile.name != ADAPTER_ID);
        config.providers.push(settings.profile(reference.clone()));
        config.active_provider = Some(ADAPTER_ID.into());
        if let Err(error) = self.store.save(config.clone()).await {
            if created && let Some(reference) = &reference {
                let _ = self.core.secrets().delete(reference).await;
            }
            return Err(error.into());
        }
        self.core.replace_config(config);
        if let Some(previous) = previous
            && Some(&previous) != reference.as_ref()
        {
            // Cleanup failure does not invalidate an already committed configuration.
            let _ = self.core.secrets().delete(&previous).await;
        }
        Ok(())
    }
}
