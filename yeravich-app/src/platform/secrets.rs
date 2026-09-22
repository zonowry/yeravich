use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures_util::lock::Mutex;
use yeravich_core::{
    CoreFuture, SecretReference, SecretStore, SecretStoreError, SecretValue,
    settings::ConfigStoreError,
};
use zeroize::Zeroizing;

use super::storage;

/// Deliberately stores plaintext credentials for the prototype.
/// The core still uses opaque references so a different store can replace this later.
pub struct FileSecretStore {
    path: Arc<PathBuf>,
    writes: Arc<Mutex<()>>,
}

impl FileSecretStore {
    pub fn new() -> Result<Self, ConfigStoreError> {
        Ok(Self {
            path: Arc::new(storage::directory()?.join("credentials.json")),
            writes: Arc::default(),
        })
    }
}

async fn read(path: &Path) -> Result<BTreeMap<String, String>, SecretStoreError> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => serde_json::from_str(&Zeroizing::new(contents))
            .map_err(|_| SecretStoreError::Unavailable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(_) => Err(SecretStoreError::Unavailable),
    }
}

async fn write(path: &Path, entries: &BTreeMap<String, String>) -> Result<(), SecretStoreError> {
    let contents = Zeroizing::new(
        serde_json::to_vec_pretty(entries).map_err(|_| SecretStoreError::Unavailable)?,
    );
    storage::atomic_write(path, &contents)
        .await
        .map_err(|_| SecretStoreError::Unavailable)
}

impl SecretStore for FileSecretStore {
    fn load(
        &self,
        reference: &SecretReference,
    ) -> CoreFuture<Result<SecretValue, SecretStoreError>> {
        let (path, reference) = (Arc::clone(&self.path), reference.clone());
        Box::pin(async move {
            read(&path)
                .await?
                .remove(&reference.0)
                .map(|value| SecretValue::new(value.into_bytes()))
                .ok_or(SecretStoreError::NotFound)
        })
    }

    fn save(
        &self,
        reference: &SecretReference,
        secret: SecretValue,
    ) -> CoreFuture<Result<(), SecretStoreError>> {
        let (path, reference, writes) = (
            Arc::clone(&self.path),
            reference.clone(),
            Arc::clone(&self.writes),
        );
        Box::pin(async move {
            let _guard = writes.lock().await;
            let value = std::str::from_utf8(secret.expose())
                .map_err(|_| SecretStoreError::Unavailable)?
                .to_owned();
            let mut entries = read(&path).await?;
            entries.insert(reference.0, value);
            write(&path, &entries).await
        })
    }

    fn delete(&self, reference: &SecretReference) -> CoreFuture<Result<(), SecretStoreError>> {
        let (path, reference, writes) = (
            Arc::clone(&self.path),
            reference.clone(),
            Arc::clone(&self.writes),
        );
        Box::pin(async move {
            let _guard = writes.lock().await;
            let mut entries = read(&path).await?;
            entries.remove(&reference.0);
            write(&path, &entries).await
        })
    }
}

#[cfg(test)]
pub struct UnavailableSecretStore;

#[cfg(test)]
impl SecretStore for UnavailableSecretStore {
    fn load(
        &self,
        _reference: &SecretReference,
    ) -> CoreFuture<Result<SecretValue, SecretStoreError>> {
        Box::pin(async { Err(SecretStoreError::Unavailable) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plaintext_credentials_survive_reopening_and_can_be_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let path = Arc::new(directory.path().join("credentials.json"));
        let store = FileSecretStore {
            path: Arc::clone(&path),
            writes: Arc::default(),
        };
        let reference = SecretReference("test-reference".into());
        let value = directory.path().to_string_lossy().into_owned();
        store
            .save(&reference, SecretValue::new(value.as_bytes().to_vec()))
            .await
            .unwrap();
        assert!(
            tokio::fs::read_to_string(path.as_ref())
                .await
                .unwrap()
                .contains(&value)
        );
        let reopened = FileSecretStore {
            path,
            writes: Arc::default(),
        };
        let loaded = reopened.load(&reference).await.unwrap();
        let matches = loaded.expose() == value.as_bytes();
        assert!(matches, "saved credential did not round-trip");
        reopened.delete(&reference).await.unwrap();
        assert!(matches!(
            reopened.load(&reference).await,
            Err(SecretStoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn malformed_credentials_are_not_silently_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let path = Arc::new(directory.path().join("credentials.json"));
        tokio::fs::write(path.as_ref(), "malformed JSON")
            .await
            .unwrap();
        let store = FileSecretStore {
            path: Arc::clone(&path),
            writes: Arc::default(),
        };
        let reference = SecretReference("test-reference".into());
        let value = directory.path().to_string_lossy().into_owned();
        assert!(
            store
                .save(&reference, SecretValue::new(value.into_bytes()))
                .await
                .is_err()
        );
        assert_eq!(
            tokio::fs::read_to_string(path.as_ref()).await.unwrap(),
            "malformed JSON"
        );
    }
}
