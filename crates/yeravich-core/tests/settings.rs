use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use yeravich_core::{
    AppConfig, CoreFuture, SecretReference, SecretStore, SecretStoreError, SecretValue, Yeravich,
    openai::{ADAPTER_ID, ChatClient, ChatSettings},
    settings::{ConfigStore, ConfigStoreError, SecretChange, SettingsService},
};

#[derive(Default)]
struct MemoryConfig {
    value: Arc<Mutex<AppConfig>>,
    fail: Arc<AtomicBool>,
}

impl ConfigStore for MemoryConfig {
    fn load(&self) -> CoreFuture<Result<AppConfig, ConfigStoreError>> {
        let config = self.value.lock().unwrap().clone();
        Box::pin(async { Ok(config) })
    }
    fn save(&self, config: AppConfig) -> CoreFuture<Result<(), ConfigStoreError>> {
        let (value, fail) = (Arc::clone(&self.value), Arc::clone(&self.fail));
        Box::pin(async move {
            if fail.load(Ordering::SeqCst) {
                return Err(ConfigStoreError::Write);
            }
            *value.lock().unwrap() = config;
            Ok(())
        })
    }
}

#[derive(Default)]
struct MemorySecrets {
    values: Arc<Mutex<BTreeMap<String, SecretValue>>>,
    fail: Arc<AtomicBool>,
}

impl SecretStore for MemorySecrets {
    fn load(
        &self,
        reference: &SecretReference,
    ) -> CoreFuture<Result<SecretValue, SecretStoreError>> {
        let result = self
            .values
            .lock()
            .unwrap()
            .get(&reference.0)
            .map(|secret| SecretValue::new(secret.expose().to_vec()))
            .ok_or(SecretStoreError::NotFound);
        Box::pin(async { result })
    }
    fn save(
        &self,
        reference: &SecretReference,
        secret: SecretValue,
    ) -> CoreFuture<Result<(), SecretStoreError>> {
        let (values, reference, fail) = (
            Arc::clone(&self.values),
            reference.clone(),
            Arc::clone(&self.fail),
        );
        Box::pin(async move {
            if fail.load(Ordering::SeqCst) {
                return Err(SecretStoreError::Unavailable);
            }
            values.lock().unwrap().insert(reference.0, secret);
            Ok(())
        })
    }
    fn delete(&self, reference: &SecretReference) -> CoreFuture<Result<(), SecretStoreError>> {
        self.values.lock().unwrap().remove(&reference.0);
        Box::pin(async { Ok(()) })
    }
}

fn settings(model: &str) -> ChatSettings {
    ChatSettings::new("http://localhost:8080/v1", model).unwrap()
}
fn replacement() -> SecretChange {
    SecretChange::Replace(SecretValue::new(
        uuid::Uuid::new_v4().to_string().into_bytes(),
    ))
}

fn setup() -> (
    Yeravich,
    SettingsService,
    Arc<MemoryConfig>,
    Arc<MemorySecrets>,
) {
    let config = Arc::new(MemoryConfig::default());
    let secrets = Arc::new(MemorySecrets::default());
    let core = Yeravich::new(AppConfig::default(), secrets.clone());
    let service = SettingsService::new(core.clone(), config.clone(), ChatClient::new().unwrap());
    (core, service, config, secrets)
}

#[tokio::test]
async fn saves_reloads_replaces_and_removes_credentials() {
    let (core, service, config, secrets) = setup();
    service
        .save(settings("first"), replacement())
        .await
        .unwrap();
    let first = service.saved_profile().unwrap().secret_ref.unwrap();
    assert_eq!(core.config().active_provider.as_deref(), Some(ADAPTER_ID));
    let encoded = config.value.lock().unwrap().to_toml().unwrap();
    let value = secrets.load(&first).await.unwrap();
    assert!(!encoded.contains(std::str::from_utf8(value.expose()).unwrap()));
    assert!(!format!("{value:?}").contains(std::str::from_utf8(value.expose()).unwrap()));

    service
        .save(settings("second"), SecretChange::Keep)
        .await
        .unwrap();
    assert_eq!(
        service.saved_profile().unwrap().secret_ref,
        Some(first.clone())
    );
    let reloaded = Yeravich::new(AppConfig::default(), secrets.clone());
    let reopened =
        SettingsService::new(reloaded.clone(), config.clone(), ChatClient::new().unwrap());
    reopened.load().await.unwrap();
    assert_eq!(reloaded.config(), core.config());

    service
        .save(settings("third"), replacement())
        .await
        .unwrap();
    assert!(secrets.load(&first).await.is_err());
    assert_eq!(secrets.values.lock().unwrap().len(), 1);
    service
        .save(settings("fourth"), SecretChange::Remove)
        .await
        .unwrap();
    assert!(secrets.values.lock().unwrap().is_empty());
    assert!(service.saved_profile().unwrap().secret_ref.is_none());
}

#[tokio::test]
async fn failed_writes_preserve_active_settings_and_the_previous_key() {
    let (core, service, config, secrets) = setup();
    service
        .save(settings("original"), replacement())
        .await
        .unwrap();
    let before = core.config();
    config.fail.store(true, Ordering::SeqCst);
    assert!(
        service
            .save(settings("unsaved"), replacement())
            .await
            .is_err()
    );
    assert_eq!(core.config(), before);
    assert_eq!(*config.value.lock().unwrap(), before);
    assert_eq!(secrets.values.lock().unwrap().len(), 1);

    config.fail.store(false, Ordering::SeqCst);
    secrets.fail.store(true, Ordering::SeqCst);
    assert!(
        service
            .save(settings("also-unsaved"), replacement())
            .await
            .is_err()
    );
    assert_eq!(core.config(), before);
    assert_eq!(*config.value.lock().unwrap(), before);
}

#[tokio::test]
async fn saving_without_an_api_key_does_not_need_credential_storage() {
    let (core, service, _, secrets) = setup();
    secrets.fail.store(true, Ordering::SeqCst);
    service
        .save(settings("local-model"), SecretChange::Keep)
        .await
        .unwrap();
    assert_eq!(core.config().active_provider.as_deref(), Some(ADAPTER_ID));
    assert!(service.saved_profile().unwrap().secret_ref.is_none());
}
