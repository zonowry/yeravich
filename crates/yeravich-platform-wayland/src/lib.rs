//! Replaceable standard Wayland integration boundaries.
//!
//! These adapters intentionally report explicit capability errors until their
//! event-loop integration is implemented. They never panic in an empty desktop
//! environment.

use std::{path::PathBuf, sync::Arc};

use yeravich_core::{
    AppConfig, Capability, CapabilityError, ConfigStore, PortFuture, SelectionReader,
    ShortcutSource, ShortcutStream,
};

#[derive(Debug, Clone)]
pub struct XdgConfigStore {
    path: PathBuf,
}

impl XdgConfigStore {
    /// Creates a store for an explicit file path, useful for tests and portable setups.
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Resolves `$XDG_CONFIG_HOME/yeravich/config.toml`, falling back to
    /// `$HOME/.config/yeravich/config.toml`.
    ///
    /// # Errors
    ///
    /// Returns a capability error if neither environment variable is available.
    pub fn discover() -> Result<Self, CapabilityError> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .ok_or_else(|| {
                CapabilityError::unavailable(
                    Capability::ConfigStorage,
                    "neither XDG_CONFIG_HOME nor HOME is set",
                )
            })?;
        Ok(Self::new(base.join("yeravich/config.toml")))
    }

    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl ConfigStore for XdgConfigStore {
    fn load(&self) -> PortFuture<Result<AppConfig, CapabilityError>> {
        let path = self.path.clone();
        Box::pin(async move {
            match std::fs::read_to_string(&path) {
                Ok(contents) => AppConfig::from_toml(&contents).map_err(|error| {
                    config_error(format!("could not parse {}: {error}", path.display()))
                }),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(AppConfig::default())
                }
                Err(error) => Err(config_error(format!(
                    "could not read {}: {error}",
                    path.display()
                ))),
            }
        })
    }

    fn save(&self, config: AppConfig) -> PortFuture<Result<(), CapabilityError>> {
        let path = self.path.clone();
        Box::pin(async move {
            let contents = config
                .to_toml()
                .map_err(|error| config_error(error.to_string()))?;
            let parent = path
                .parent()
                .ok_or_else(|| config_error("configuration path has no parent directory"))?;
            std::fs::create_dir_all(parent).map_err(|error| {
                config_error(format!("could not create {}: {error}", parent.display()))
            })?;
            let temporary = path.with_extension("toml.tmp");
            std::fs::write(&temporary, contents).map_err(|error| {
                config_error(format!("could not write {}: {error}", temporary.display()))
            })?;
            std::fs::rename(&temporary, &path).map_err(|error| {
                config_error(format!("could not replace {}: {error}", path.display()))
            })
        })
    }
}

fn config_error(message: impl Into<String>) -> CapabilityError {
    CapabilityError::unavailable(Capability::ConfigStorage, message)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PortalShortcutSource;

impl ShortcutSource for PortalShortcutSource {
    fn activations(&self) -> Result<ShortcutStream, CapabilityError> {
        Err(CapabilityError::unavailable(
            Capability::GlobalShortcutsPortal,
            "portal registration is not implemented in this scaffold",
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct IpcActivationSource;

impl ShortcutSource for IpcActivationSource {
    fn activations(&self) -> Result<ShortcutStream, CapabilityError> {
        Err(CapabilityError::unavailable(
            Capability::SingleInstanceIpc,
            "single-instance activation IPC is not implemented in this scaffold",
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AtSpiSelectionReader;

impl SelectionReader for AtSpiSelectionReader {
    fn read_selection(&self) -> PortFuture<Result<String, CapabilityError>> {
        Box::pin(async {
            Err(CapabilityError::unavailable(
                Capability::AtSpiSelection,
                "AT-SPI selection reading is not implemented in this scaffold",
            ))
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PrimarySelectionReader;

impl SelectionReader for PrimarySelectionReader {
    fn read_selection(&self) -> PortFuture<Result<String, CapabilityError>> {
        Box::pin(async {
            Err(CapabilityError::unavailable(
                Capability::PrimarySelection,
                "Wayland PRIMARY selection reading is not implemented in this scaffold",
            ))
        })
    }
}

/// Reads through standard interfaces in order, invoking the compositor-specific
/// fallback only after both standard readers fail.
pub struct OrderedSelectionReader {
    atspi: Arc<dyn SelectionReader>,
    primary: Arc<dyn SelectionReader>,
    fallback: Arc<dyn SelectionReader>,
}

impl std::fmt::Debug for OrderedSelectionReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OrderedSelectionReader")
    }
}

impl OrderedSelectionReader {
    #[must_use]
    pub fn new(
        atspi: Arc<dyn SelectionReader>,
        primary: Arc<dyn SelectionReader>,
        fallback: Arc<dyn SelectionReader>,
    ) -> Self {
        Self {
            atspi,
            primary,
            fallback,
        }
    }
}

impl SelectionReader for OrderedSelectionReader {
    fn read_selection(&self) -> PortFuture<Result<String, CapabilityError>> {
        let atspi = Arc::clone(&self.atspi);
        let primary = Arc::clone(&self.primary);
        let fallback = Arc::clone(&self.fallback);
        Box::pin(async move {
            if let Ok(selection) = atspi.read_selection().await {
                return Ok(selection);
            }
            if let Ok(selection) = primary.read_selection().await {
                return Ok(selection);
            }
            fallback.read_selection().await.map_err(|last_error| {
                CapabilityError::unavailable(
                    Capability::HyprlandFallback,
                    format!("all selection methods failed; final error: {last_error}"),
                )
            })
        })
    }
}

/// Confirms the native dependency set at compile time without exposing those
/// implementation types through the public core API.
#[must_use]
pub const fn native_dependency_baseline() -> (&'static str, &'static str, &'static str) {
    ("ashpd-0.12", "atspi-0.30", "wl-clipboard-rs-0.9")
}
