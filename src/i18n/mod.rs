#[cfg(test)]
mod tests;

use std::fmt;

include!(concat!(env!("OUT_DIR"), "/langs.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Chinese,
    English,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::Chinese, Self::English];

    pub fn locale(self) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
        }
    }

    pub fn from_locale(locale: &str) -> Self {
        let language = locale
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default();

        if language.eq_ignore_ascii_case("zh") {
            Self::Chinese
        } else {
            Self::English
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::Chinese => Self::English,
            Self::English => Self::Chinese,
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        sys_locale::get_locale()
            .as_deref()
            .map(Self::from_locale)
            .unwrap_or(Self::English)
    }
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Chinese => "简体中文",
            Self::English => "English",
        })
    }
}
