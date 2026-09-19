//! Platform-independent contracts and state transitions for Yeravich.

mod config;
mod domain;
mod ports;
mod reducer;

pub use config::{AppConfig, CONFIG_VERSION, ConfigError};
pub use domain::{
    Capability, CapabilityError, CapabilityState, LanguagePair, ProviderAdapterDescriptor,
    ProviderProfile, RequestId, SecretReference, TranslationEvent, TranslationRequest,
};
pub use ports::{
    AdapterRegistry, ConfigStore, PortFuture, SecretStore, SelectionReader, ShortcutSource,
    ShortcutStream, TranslationProvider, TranslationStream,
};
pub use reducer::{AppEvent, AppPhase, AppState, Effect, reduce};
