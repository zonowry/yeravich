use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::AppConfig;

pub type CoreFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePair {
    pub source: String,
    pub target: String,
}

impl Default for LanguagePair {
    fn default() -> Self {
        Self {
            source: "en".into(),
            target: "zh-CN".into(),
        }
    }
}

/// Opaque locator for a secret held by the platform's credential store.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretReference(pub String);

impl std::fmt::Debug for SecretReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretReference([redacted])")
    }
}

/// Secret material deliberately has no serialization or cloning support.
pub struct SecretValue(Vec<u8>);

impl SecretValue {
    #[must_use]
    pub fn new(value: Vec<u8>) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretValue([redacted])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub name: String,
    pub adapter_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub public: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<SecretReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub text: String,
    pub languages: LanguagePair,
    pub profile: ProviderProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub source: String,
    pub text: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretStoreError {
    #[error("the platform secret store is unavailable")]
    Unavailable,
    #[error("the requested credential was not found")]
    NotFound,
    #[error("the platform denied access to the credential")]
    PermissionDenied,
}

pub trait SecretStore: Send + Sync {
    fn load(
        &self,
        reference: &SecretReference,
    ) -> CoreFuture<Result<SecretValue, SecretStoreError>>;
}

pub trait TranslationBackend: Send + Sync {
    fn translate(
        &self,
        request: TranslationRequest,
        secret: Option<SecretValue>,
    ) -> CoreFuture<Result<Translation, TranslationError>>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TranslationError {
    #[error("enter some text to translate")]
    EmptyText,
    #[error("no translation provider is configured")]
    NoProviderConfigured,
    #[error("the active provider profile does not exist")]
    ActiveProviderMissing,
    #[error("translation adapter '{0}' is not installed")]
    AdapterUnavailable(String),
    #[error("could not load the provider credential: {0}")]
    Credential(SecretStoreError),
    #[error("the provider rejected the request: {0}")]
    Provider(String),
}

struct Inner {
    config: RwLock<AppConfig>,
    backends: RwLock<HashMap<String, Arc<dyn TranslationBackend>>>,
    secrets: Arc<dyn SecretStore>,
}

/// Directly callable Yeravich product capabilities, independent of any UI toolkit.
#[derive(Clone)]
pub struct Yeravich {
    inner: Arc<Inner>,
}

impl Yeravich {
    #[must_use]
    pub fn new(config: AppConfig, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            inner: Arc::new(Inner {
                config: RwLock::new(config),
                backends: RwLock::new(HashMap::new()),
                secrets,
            }),
        }
    }

    /// Adds or replaces a translation implementation under a stable adapter ID.
    pub fn register_backend(
        &self,
        adapter_id: impl Into<String>,
        backend: Arc<dyn TranslationBackend>,
    ) {
        self.inner
            .backends
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(adapter_id.into(), backend);
    }

    #[must_use]
    pub fn config(&self) -> AppConfig {
        self.inner
            .config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_languages(&self, source: impl Into<String>, target: impl Into<String>) {
        self.inner
            .config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_languages(source, target);
    }

    pub fn swap_languages(&self) {
        self.inner
            .config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .swap_languages();
    }

    /// Translates text with the active profile. Provider and credential details stay hidden here.
    ///
    /// # Errors
    ///
    /// Returns [`TranslationError`] when the input or active profile is invalid, a required
    /// capability is unavailable, or the selected provider fails.
    pub async fn translate_text(
        &self,
        text: impl Into<String>,
    ) -> Result<Translation, TranslationError> {
        let text = text.into();
        let text = text.trim();
        if text.is_empty() {
            return Err(TranslationError::EmptyText);
        }

        let config = self.config();
        let active = config
            .active_provider
            .ok_or(TranslationError::NoProviderConfigured)?;
        let profile = config
            .providers
            .into_iter()
            .find(|profile| profile.name == active)
            .ok_or(TranslationError::ActiveProviderMissing)?;
        let adapter = self
            .inner
            .backends
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&profile.adapter_id)
            .cloned()
            .ok_or_else(|| TranslationError::AdapterUnavailable(profile.adapter_id.clone()))?;
        let secret = match &profile.secret_ref {
            Some(reference) => Some(
                self.inner
                    .secrets
                    .load(reference)
                    .await
                    .map_err(TranslationError::Credential)?,
            ),
            None => None,
        };

        adapter
            .translate(
                TranslationRequest {
                    text: text.to_owned(),
                    languages: config.languages,
                    profile,
                },
                secret,
            )
            .await
    }
}
