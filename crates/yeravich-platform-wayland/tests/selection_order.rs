use std::sync::{Arc, Mutex};

use yeravich_core::{Capability, CapabilityError, PortFuture, SelectionReader};
use yeravich_platform_wayland::OrderedSelectionReader;

struct FakeReader {
    name: &'static str,
    result: Result<&'static str, CapabilityError>,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl SelectionReader for FakeReader {
    fn read_selection(&self) -> PortFuture<Result<String, CapabilityError>> {
        self.calls.lock().unwrap().push(self.name);
        let result = self.result.clone().map(str::to_owned);
        Box::pin(async move { result })
    }
}

fn failure(capability: Capability) -> Result<&'static str, CapabilityError> {
    Err(CapabilityError::unavailable(capability, "unavailable"))
}

#[test]
fn standard_reader_success_prevents_later_fallbacks() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = OrderedSelectionReader::new(
        Arc::new(FakeReader {
            name: "atspi",
            result: Ok("selected"),
            calls: Arc::clone(&calls),
        }),
        Arc::new(FakeReader {
            name: "primary",
            result: Ok("wrong"),
            calls: Arc::clone(&calls),
        }),
        Arc::new(FakeReader {
            name: "hyprland",
            result: Ok("wrong"),
            calls: Arc::clone(&calls),
        }),
    );
    let selected = futures_executor::block_on(reader.read_selection()).unwrap();
    assert_eq!(selected, "selected");
    assert_eq!(*calls.lock().unwrap(), ["atspi"]);
}

#[test]
fn hyprland_runs_only_after_both_standard_readers_fail() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = OrderedSelectionReader::new(
        Arc::new(FakeReader {
            name: "atspi",
            result: failure(Capability::AtSpiSelection),
            calls: Arc::clone(&calls),
        }),
        Arc::new(FakeReader {
            name: "primary",
            result: failure(Capability::PrimarySelection),
            calls: Arc::clone(&calls),
        }),
        Arc::new(FakeReader {
            name: "hyprland",
            result: Ok("fallback"),
            calls: Arc::clone(&calls),
        }),
    );
    let selected = futures_executor::block_on(reader.read_selection()).unwrap();
    assert_eq!(selected, "fallback");
    assert_eq!(*calls.lock().unwrap(), ["atspi", "primary", "hyprland"]);
}
