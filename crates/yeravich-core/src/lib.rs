//! UI-independent product capabilities for Yeravich.

mod config;
pub mod openai;
pub mod settings;
mod translation;

pub use config::{AppConfig, CONFIG_VERSION, ConfigError};
pub use translation::{
    CoreFuture, LanguagePair, ProviderProfile, SecretReference, SecretStore, SecretStoreError,
    SecretValue, Translation, TranslationBackend, TranslationError, TranslationRequest, Yeravich,
};
