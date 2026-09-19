use std::{future::Future, pin::Pin};

use futures_core::Stream;

use crate::{
    AppConfig, CapabilityError, ProviderAdapterDescriptor, SecretReference, TranslationEvent,
    TranslationRequest,
};

pub type PortFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type ShortcutStream = Pin<Box<dyn Stream<Item = ()> + Send + 'static>>;
pub type TranslationStream =
    Pin<Box<dyn Stream<Item = Result<TranslationEvent, CapabilityError>> + Send + 'static>>;

pub trait ShortcutSource: Send + Sync {
    /// Opens the long-lived shortcut activation stream.
    ///
    /// # Errors
    ///
    /// Returns a capability error when the source cannot be registered.
    fn activations(&self) -> Result<ShortcutStream, CapabilityError>;
}

pub trait SelectionReader: Send + Sync {
    fn read_selection(&self) -> PortFuture<Result<String, CapabilityError>>;
}

pub trait TranslationProvider: Send + Sync {
    /// Starts a translation event stream for one request.
    ///
    /// # Errors
    ///
    /// Returns a capability error when the adapter cannot start the request.
    fn translate(&self, request: TranslationRequest) -> Result<TranslationStream, CapabilityError>;
}

pub trait SecretStore: Send + Sync {
    fn load(&self, reference: &SecretReference) -> PortFuture<Result<Vec<u8>, CapabilityError>>;
    fn save(&self, secret: Vec<u8>) -> PortFuture<Result<SecretReference, CapabilityError>>;
    fn delete(&self, reference: &SecretReference) -> PortFuture<Result<(), CapabilityError>>;
}

pub trait ConfigStore: Send + Sync {
    fn load(&self) -> PortFuture<Result<AppConfig, CapabilityError>>;
    fn save(&self, config: AppConfig) -> PortFuture<Result<(), CapabilityError>>;
}

/// Lightweight registry of adapters linked by the final application.
#[derive(Debug, Clone, Copy)]
pub struct AdapterRegistry {
    adapters: &'static [ProviderAdapterDescriptor],
}

impl AdapterRegistry {
    #[must_use]
    pub const fn new(adapters: &'static [ProviderAdapterDescriptor]) -> Self {
        Self { adapters }
    }

    #[must_use]
    pub fn find(&self, id: &str) -> Option<&'static ProviderAdapterDescriptor> {
        self.adapters.iter().find(|adapter| adapter.id == id)
    }

    #[must_use]
    pub const fn all(&self) -> &'static [ProviderAdapterDescriptor] {
        self.adapters
    }
}
