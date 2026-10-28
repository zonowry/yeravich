use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use reqwest::{Url, header::HeaderValue};
use rust_i18n::t;
use serde::Deserialize;

use crate::{i18n::Langs, shortcut::Binding};

#[cfg(test)]
mod tests;

const DEFAULT_CONFIG: &str = include_str!("../config.example.toml");

pub struct Config {
    pub translation: TranslationConfig,
    pub show_window: Binding,
}

// 不派生 Debug，避免在日志中意外输出 API Key。
#[derive(Clone)]
pub struct TranslationConfig {
    pub endpoint: Url,
    pub model: String,
    pub authorization: Option<HeaderValue>,
}

#[derive(Debug, Clone)]
pub enum Error {
    PathUnavailable,
    Missing(PathBuf),
    Read(PathBuf),
    Create(PathBuf),
    Invalid { path: PathBuf, field: &'static str },
}

impl Error {
    pub fn message(&self, locale: &str) -> String {
        match self {
            Self::PathUnavailable => {
                t!(Langs::CONFIG_PATH_UNAVAILABLE, locale = locale).into_owned()
            }
            Self::Missing(path) => t!(
                Langs::CONFIG_MISSING,
                locale = locale,
                path = path.display()
            )
            .into_owned(),
            Self::Read(path) => t!(
                Langs::CONFIG_READ_ERROR,
                locale = locale,
                path = path.display()
            )
            .into_owned(),
            Self::Create(path) => t!(
                Langs::CONFIG_CREATE_ERROR,
                locale = locale,
                path = path.display()
            )
            .into_owned(),
            Self::Invalid { path, field } => t!(
                Langs::CONFIG_INVALID,
                locale = locale,
                path = path.display(),
                field = field
            )
            .into_owned(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    translation: RawTranslationConfig,
    #[serde(default)]
    shortcuts: RawShortcuts,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShortcuts {
    show_window: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTranslationConfig {
    endpoint: String,
    model: String,
    api_key: Option<String>,
}

pub async fn load() -> Result<Config, Error> {
    load_with_paths(std::env::var_os("YERAVICH_CONFIG"), dirs::config_dir()).await
}

async fn load_with_paths(
    override_path: Option<OsString>,
    base: Option<PathBuf>,
) -> Result<Config, Error> {
    let use_default = override_path.as_ref().is_none_or(|path| path.is_empty());
    let path = resolve_path(override_path, base)?;
    match load_from(&path).await {
        Err(Error::Missing(_)) if use_default => {
            // 阻塞任务需要拥有路径；外层仍保留 path，用于错误提示和重新读取。
            let destination = path.clone();
            tokio::task::spawn_blocking(move || create_default(&destination))
                .await
                .map_err(|_| Error::Create(path.clone()))?
                .map_err(|_| Error::Create(path.clone()))?;
            load_from(&path).await
        }
        result => result,
    }
}

fn create_default(path: &Path) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config has no parent"))?;
    std::fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(DEFAULT_CONFIG.as_bytes())?;
    // 完整写入后才发布文件；另一个进程先创建时保留它的内容，再由调用方读取。
    match file.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}

fn resolve_path(override_path: Option<OsString>, base: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(path) = override_path.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    base.map(|base| base.join("yeravich").join("config.toml"))
        .ok_or(Error::PathUnavailable)
}

async fn load_from(path: &Path) -> Result<Config, Error> {
    let contents = tokio::fs::read_to_string(path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Missing(path.to_owned())
        } else {
            Error::Read(path.to_owned())
        }
    })?;
    parse(&contents).map_err(|field| Error::Invalid {
        path: path.to_owned(),
        field,
    })
}

fn parse(contents: &str) -> Result<Config, &'static str> {
    // TOML 的原始错误可能包含带密钥的源代码行，只向界面返回字段名称。
    let file = toml::from_str::<FileConfig>(contents).map_err(
        |_| "TOML / [translation] / endpoint / model / api_key / [shortcuts] / show_window",
    )?;
    let show_window = file
        .shortcuts
        .show_window
        .as_deref()
        .map(Binding::parse)
        .transpose()?
        .unwrap_or_default();
    let raw = file.translation;
    let endpoint = Url::parse(raw.endpoint.trim()).map_err(|_| "endpoint")?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.fragment().is_some()
    {
        return Err("endpoint");
    }
    let model = raw.model.trim().to_owned();
    if model.is_empty() {
        return Err("model");
    }
    let authorization = raw
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| -> Result<HeaderValue, &'static str> {
            let mut value =
                HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| "api_key")?;
            value.set_sensitive(true);
            Ok(value)
        })
        .transpose()?;

    Ok(Config {
        translation: TranslationConfig {
            endpoint,
            model,
            authorization,
        },
        show_window,
    })
}
