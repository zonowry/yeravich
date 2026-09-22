use std::{collections::BTreeMap, sync::Arc};

use futures_executor::block_on;
use futures_util::future::Aborted;
use yeravich_core::{
    AppConfig, CoreFuture, ProviderProfile, SecretValue, Translation, TranslationBackend,
    TranslationError, TranslationRequest, Yeravich,
};

use super::{Completion, PendingTranslation, TranslationController};
use crate::platform::secrets::UnavailableSecretStore;

struct Echo;

impl TranslationBackend for Echo {
    fn translate(
        &self,
        request: TranslationRequest,
        _secret: Option<SecretValue>,
    ) -> CoreFuture<Result<Translation, TranslationError>> {
        Box::pin(async move {
            Ok(Translation {
                source: request.text.clone(),
                text: format!("{}: {}", request.languages.target, request.text),
            })
        })
    }
}

fn controller(with_provider: bool) -> TranslationController {
    let mut config = AppConfig::default();
    if with_provider {
        config.active_provider = Some("test".into());
        config.providers.push(ProviderProfile {
            name: "test".into(),
            adapter_id: "echo".into(),
            public: BTreeMap::default(),
            secret_ref: None,
        });
    }
    let core = Yeravich::new(config, Arc::new(UnavailableSecretStore));
    core.register_backend("echo", Arc::new(Echo));
    TranslationController::new(core)
}

fn finish(pending: PendingTranslation) -> Completion {
    Completion {
        id: pending.id,
        result: block_on(pending.future).expect("task was not cancelled"),
    }
}

#[test]
fn translates_using_swapped_languages_and_restores_idle_state() {
    let mut controller = controller(true);
    controller.swap_languages();
    assert_eq!(controller.state().languages.source, "zh-CN");
    assert_eq!(controller.state().languages.target, "en");
    controller.set_source(" hello\nworld ".into());
    let pending = controller.translate().unwrap();
    assert!(controller.state().busy);
    controller.swap_languages();
    controller.set_source("ignored while busy".into());
    assert_eq!(controller.state().source_text, " hello\nworld ");
    controller.complete(finish(pending));

    assert!(!controller.state().busy);
    assert_eq!(controller.state().translated_text, "en: hello\nworld");
    assert_eq!(controller.state().status, "Done");
}

#[test]
fn missing_provider_reports_error_and_allows_retry() {
    let mut controller = controller(false);
    controller.set_source("hello".into());
    let pending = controller.translate().unwrap();
    controller.complete(finish(pending));

    assert!(!controller.state().busy);
    assert_eq!(
        controller.state().status,
        "no translation provider is configured"
    );
    assert!(controller.state().translated_text.is_empty());
    assert!(controller.translate().is_some());
}

#[test]
fn whitespace_does_not_start_work() {
    let mut controller = controller(true);
    controller.set_source(" \n\t ".into());
    assert!(controller.translate().is_none());
    assert!(!controller.state().busy);
}

#[test]
fn queued_old_completion_cannot_overwrite_a_replacement() {
    let mut controller = controller(true);
    controller.set_source("hello".into());
    let old = finish(controller.translate().unwrap());
    let replacement = controller.translate().unwrap();
    controller.complete(old.clone());
    assert!(controller.state().busy);
    assert!(controller.state().translated_text.is_empty());

    controller.complete(finish(replacement));
    controller.complete(Completion {
        id: old.id,
        result: Err(TranslationError::NoProviderConfigured),
    });
    assert!(!controller.state().busy);
    assert_eq!(controller.state().translated_text, "zh-CN: hello");
    assert_eq!(controller.state().status, "Done");
}

#[test]
fn replacing_and_dropping_controller_cancel_owned_requests() {
    let mut controller = controller(true);
    controller.set_source("hello".into());
    let old = controller.translate().unwrap();
    let replacement = controller.translate().unwrap();
    assert_eq!(block_on(old.future), Err(Aborted));

    drop(controller);
    assert_eq!(block_on(replacement.future), Err(Aborted));
}
