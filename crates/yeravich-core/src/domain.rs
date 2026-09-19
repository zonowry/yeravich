use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Monotonically increasing identifier used to reject stale asynchronous work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub u64);

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

/// Opaque locator for a secret held outside the configuration file.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretReference(pub String);

impl std::fmt::Debug for SecretReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretReference([redacted])")
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
    pub id: RequestId,
    pub text: String,
    pub languages: LanguagePair,
    pub provider: ProviderProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationEvent {
    Delta { id: RequestId, text: String },
    Completed { id: RequestId, text: String },
}

impl TranslationEvent {
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        match self {
            Self::Delta { id, .. } | Self::Completed { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    GlobalShortcutsPortal,
    AtSpiSelection,
    PrimarySelection,
    HyprlandFallback,
    SingleInstanceIpc,
    ConfigStorage,
    SecretStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityState {
    Unknown,
    Available,
    Unavailable(String),
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{capability:?} is unavailable: {message}")]
pub struct CapabilityError {
    pub capability: Capability,
    pub message: String,
}

impl CapabilityError {
    #[must_use]
    pub fn unavailable(capability: Capability, message: impl Into<String>) -> Self {
        Self {
            capability,
            message: message.into(),
        }
    }
}

/// Descriptor placed in a compile-time registry by the assembly crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderAdapterDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
}
