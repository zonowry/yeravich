use std::{collections::BTreeMap, sync::Arc};

use futures_util::FutureExt;
use yeravich_core::{
    AppConfig, CoreFuture, ProviderProfile, SecretReference, SecretStore, SecretStoreError,
    SecretValue, Translation, TranslationBackend, TranslationError, TranslationRequest, Yeravich,
};

struct MemorySecrets;

impl SecretStore for MemorySecrets {
    fn load(
        &self,
        _reference: &SecretReference,
    ) -> CoreFuture<Result<SecretValue, SecretStoreError>> {
        async { Ok(SecretValue::new(b"test-only".to_vec())) }.boxed()
    }
}

struct Uppercase;

impl TranslationBackend for Uppercase {
    fn translate(
        &self,
        request: TranslationRequest,
        secret: Option<SecretValue>,
    ) -> CoreFuture<Result<Translation, TranslationError>> {
        async move {
            assert_eq!(secret.expect("credential").expose(), b"test-only");
            Ok(Translation {
                source: request.text.clone(),
                text: request.text.to_uppercase(),
            })
        }
        .boxed()
    }
}

#[test]
fn translates_through_a_registered_capability() {
    let config = AppConfig {
        active_provider: Some("test".into()),
        providers: vec![ProviderProfile {
            name: "test".into(),
            adapter_id: "uppercase".into(),
            public: BTreeMap::default(),
            secret_ref: Some(SecretReference("memory:test".into())),
        }],
        ..AppConfig::default()
    };
    let core = Yeravich::new(config, Arc::new(MemorySecrets));
    core.register_backend("uppercase", Arc::new(Uppercase));

    let result = futures_executor::block_on(core.translate_text(" hello ")).unwrap();

    assert_eq!(result.source, "hello");
    assert_eq!(result.text, "HELLO");
}

#[test]
fn reports_missing_provider_without_an_event_loop() {
    let core = Yeravich::new(AppConfig::default(), Arc::new(MemorySecrets));

    let error = futures_executor::block_on(core.translate_text("hello")).unwrap_err();

    assert_eq!(error, TranslationError::NoProviderConfigured);
}
