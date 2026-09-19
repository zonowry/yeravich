//! Optional Hyprland-specific compatibility adapters.
//!
//! The core and standard Wayland adapter do not depend on this crate. The
//! desktop assembly may inject this implementation as the final fallback.

use yeravich_core::{Capability, CapabilityError, PortFuture, SelectionReader};

#[derive(Debug, Default, Clone, Copy)]
pub struct HyprlandCopySelectionReader;

impl SelectionReader for HyprlandCopySelectionReader {
    fn read_selection(&self) -> PortFuture<Result<String, CapabilityError>> {
        Box::pin(async {
            Err(CapabilityError::unavailable(
                Capability::HyprlandFallback,
                "Hyprland copy-key fallback is not implemented in this scaffold",
            ))
        })
    }
}
