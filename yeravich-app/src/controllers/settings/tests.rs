use std::sync::Arc;

use futures_executor::block_on;
use futures_util::future::Aborted;
use yeravich_core::{
    AppConfig, CoreFuture, Yeravich,
    openai::ChatClient,
    settings::{ConfigStore, ConfigStoreError},
};

use super::*;
use crate::platform::secrets::UnavailableSecretStore;

struct MemoryConfig;

impl ConfigStore for MemoryConfig {
    fn load(&self) -> CoreFuture<Result<AppConfig, ConfigStoreError>> {
        Box::pin(async { Ok(AppConfig::default()) })
    }
    fn save(&self, _config: AppConfig) -> CoreFuture<Result<(), ConfigStoreError>> {
        Box::pin(async { Ok(()) })
    }
}

fn controller() -> SettingsController {
    let core = Yeravich::new(AppConfig::default(), Arc::new(UnavailableSecretStore));
    let service = SettingsService::new(core, Arc::new(MemoryConfig), ChatClient::new().unwrap());
    let mut controller = SettingsController::new(service);
    controller.set_address("http://localhost:8080/v1".into());
    controller.set_model("example-model".into());
    controller
}

#[test]
fn editing_cancels_detection_and_ignores_its_queued_result() {
    let mut controller = controller();
    let pending = controller.start(Activity::Testing).unwrap();
    controller.set_model("new-model".into());
    assert!(block_on(pending.future).is_err());
    controller.complete(Completion {
        id: pending.id,
        outcome: Outcome::Tested(Ok(ConnectionReport {
            elapsed: std::time::Duration::ZERO,
        })),
    });
    assert_eq!(controller.state().status, "Unsaved changes");
    assert_eq!(controller.state().model, "new-model");
    assert!(controller.state().activity == Activity::Idle);
}

#[test]
fn repeated_tests_and_leaving_the_page_cancel_previous_work() {
    let mut controller = controller();
    let old = controller.start(Activity::Testing).unwrap();
    let current = controller.start(Activity::Testing).unwrap();
    assert!(matches!(block_on(old.future), Err(Aborted)));
    controller.leave();
    assert!(matches!(block_on(current.future), Err(Aborted)));
    assert!(controller.state().activity == Activity::Idle);
    assert_eq!(controller.state().status, "Connection test cancelled");
}

#[test]
fn save_keeps_the_draft_stable_until_it_completes() {
    let mut controller = controller();
    let pending = controller.start(Activity::Saving).unwrap();
    controller.set_model("ignored".into());
    assert!(controller.start(Activity::Testing).is_none());
    assert!(controller.start(Activity::Saving).is_none());
    assert_eq!(controller.state().model, "example-model");
    let outcome = block_on(pending.future).unwrap();
    controller.complete(Completion {
        id: pending.id,
        outcome,
    });
    assert_eq!(controller.state().status, "Settings saved");
    assert!(controller.state().activity == Activity::Idle);
    assert!(controller.state().key.as_str().is_empty());
}

#[test]
fn invalid_fields_do_not_start_work_and_secret_debug_is_redacted() {
    let mut controller = controller();
    controller.set_model(" \n ".into());
    assert!(controller.start(Activity::Testing).is_none());
    assert!(controller.state().error);
    let secret = SecretInput::new(
        tempfile::NamedTempFile::new()
            .unwrap()
            .path()
            .to_string_lossy()
            .into_owned(),
    );
    assert_eq!(format!("{secret:?}"), "SecretInput([redacted])");
    let message = crate::pages::settings::Message::KeyChanged(secret);
    assert_eq!(format!("{message:?}"), "SettingsMessage([redacted])");
}

#[test]
fn dropping_settings_cancels_pending_work() {
    let mut controller = controller();
    let pending = controller.start(Activity::Testing).unwrap();
    drop(controller);
    assert!(matches!(block_on(pending.future), Err(Aborted)));
}
