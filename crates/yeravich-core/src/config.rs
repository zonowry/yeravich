use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{LanguagePair, ProviderProfile};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub version: u32,
    pub languages: LanguagePair,
    pub active_provider: Option<String>,
    pub providers: Vec<ProviderProfile>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            languages: LanguagePair::default(),
            active_provider: None,
            providers: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Serializes public settings and opaque secret references, never secret values.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Serialize`] if a public configuration value cannot
    /// be represented as TOML.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(ConfigError::Serialize)
    }

    /// Parses a configuration in the current schema version.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed TOML, unknown fields, or unsupported
    /// schema versions.
    pub fn from_toml(input: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(input).map_err(ConfigError::Deserialize)?;
        if config.version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(config.version));
        }
        Ok(config)
    }

    /// Changes the active language pair without involving a UI state machine.
    pub fn set_languages(&mut self, source: impl Into<String>, target: impl Into<String>) {
        self.languages = LanguagePair {
            source: source.into(),
            target: target.into(),
        };
    }

    /// Exchanges the source and target languages.
    pub fn swap_languages(&mut self) {
        std::mem::swap(&mut self.languages.source, &mut self.languages.target);
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not serialize configuration: {0}")]
    Serialize(#[source] toml::ser::Error),
    #[error("could not parse configuration: {0}")]
    Deserialize(#[source] toml::de::Error),
    #[error("unsupported configuration version {0}")]
    UnsupportedVersion(u32),
}
