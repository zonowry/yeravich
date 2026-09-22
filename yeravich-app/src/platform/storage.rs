use std::{
    io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use tokio::io::AsyncWriteExt;
use yeravich_core::settings::ConfigStoreError;

pub fn directory() -> Result<PathBuf, ConfigStoreError> {
    ProjectDirs::from("com", "zonowry", "yeravich")
        .map(|directories| directories.config_dir().to_owned())
        .ok_or(ConfigStoreError::Unavailable)
}

/// Replaces a complete file only after its contents have been flushed to disk.
pub async fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    tokio::fs::create_dir_all(parent).await?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut file = tokio::fs::File::from_std(temporary.reopen()?);
    file.write_all(contents).await?;
    file.sync_all().await?;
    drop(file);
    temporary.persist(path)?;
    Ok(())
}
