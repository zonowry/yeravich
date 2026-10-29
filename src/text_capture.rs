//! 启动时读取比较快照，快捷键唤出时读取文本；不模拟复制，也不修改剪贴板。
use iced::Task;
use rust_i18n::t;

use crate::i18n::{Langs, Language};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NoText,
    #[cfg(target_os = "macos")]
    AccessibilityRequired,
    #[cfg(target_os = "macos")]
    AccessibilitySettings,
    #[cfg(target_os = "macos")]
    SelectionUnavailable,
    #[cfg(target_os = "macos")]
    ClipboardFallback,
    // 这两类错误目前只由 Linux 的外部读取工具产生。
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    MissingTool,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Unavailable,
}

impl Error {
    pub fn message(self, language: Language) -> String {
        let key = match self {
            Self::NoText => Langs::CAPTURE_NO_TEXT,
            #[cfg(target_os = "macos")]
            Self::AccessibilityRequired => Langs::CAPTURE_ACCESSIBILITY_REQUIRED,
            #[cfg(target_os = "macos")]
            Self::AccessibilitySettings => Langs::CAPTURE_ACCESSIBILITY_SETTINGS,
            #[cfg(target_os = "macos")]
            Self::SelectionUnavailable => Langs::CAPTURE_SELECTION_UNAVAILABLE,
            #[cfg(target_os = "macos")]
            Self::ClipboardFallback => Langs::CAPTURE_CLIPBOARD_FALLBACK,
            Self::MissingTool => Langs::CAPTURE_MISSING_TOOL,
            Self::Unavailable => Langs::CAPTURE_UNAVAILABLE,
        };
        t!(key, locale = language.locale()).into_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Primary,
    Clipboard,
}

#[derive(Clone)]
pub struct Snapshot {
    pub primary: Result<String, Error>,
    pub clipboard: Result<String, Error>,
}

// 消息需要 Debug，但日志中不能意外展开用户的剪贴板正文。
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("primary_bytes", &self.primary.as_ref().map(String::len))
            .field("clipboard_bytes", &self.clipboard.as_ref().map(String::len))
            .finish()
    }
}

impl Snapshot {
    pub fn text(&self, source: Source) -> Result<&str, Error> {
        match source {
            Source::Primary => &self.primary,
            Source::Clipboard => &self.clipboard,
        }
        .as_deref()
        .map_err(|error| *error)
    }

    pub fn select(&self, previous: Option<&Self>, last_source: Option<Source>) -> Source {
        // Mac 的 AX 是当前来源应用的实际选区，不是可能残留的 PRIMARY。
        // 即使普通剪贴板刚变化，也优先使用用户正在选中的文字。
        if cfg!(target_os = "macos") && self.primary.is_ok() {
            return Source::Primary;
        }
        // 只在启动和唤出时采样，不持续监听剪贴板；比较内容变化而非猜测选区是否仍高亮。
        if let Some(previous) = previous {
            for source in [Source::Primary, Source::Clipboard] {
                let current = self.text(source);
                if current.is_ok() && current != previous.text(source) {
                    return source;
                }
            }
            if let Some(source) = last_source
                && self.text(source).is_ok()
            {
                return source;
            }
        }
        if self.primary.is_ok() {
            Source::Primary
        } else {
            Source::Clipboard
        }
    }
}

pub fn read(wayland: bool) -> Task<Snapshot> {
    #[cfg(target_os = "linux")]
    {
        // Xwayland 也可使用会话的 Wayland 选区；若未提供连接地址，
        // 则复用已有 X11 窗口的剪贴板，不能仅凭 XDG_SESSION_TYPE 启动 wl-paste。
        if reads_without_focus(wayland) {
            // 最后一个 Wayland 窗口关闭后 Iced 会断开剪贴板连接，
            // 独立读取不依赖窗口是否已重建或取得焦点。
            return Task::perform(wayland::read("wl-paste".as_ref()), std::convert::identity);
        }
        iced::clipboard::read_primary().then(|mut primary| {
            iced::clipboard::read().map(move |clipboard| Snapshot {
                primary: usable_text(primary.take()),
                clipboard: usable_text(clipboard),
            })
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = wayland;
        iced::clipboard::read().map(|clipboard| Snapshot {
            primary: Err(Error::NoText),
            clipboard: usable_text(clipboard),
        })
    }
}

pub fn reads_without_focus(wayland: bool) -> bool {
    #[cfg(target_os = "linux")]
    {
        wayland || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = wayland;
        // Mac 窗口隐藏后剪贴板连接仍在；AX 使用唤出前固定的 PID。
        cfg!(target_os = "macos")
    }
}

fn usable_text(text: Option<String>) -> Result<String, Error> {
    text.filter(|text| !text.trim().is_empty())
        .ok_or(Error::NoText)
}

#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_ignored_but_nonempty_text_keeps_its_whitespace() {
        for text in [None, Some(String::new()), Some(" \r\n\t".into())] {
            assert_eq!(usable_text(text), Err(Error::NoText));
        }
        assert_eq!(
            usable_text(Some("  中文\ntext\n".into())),
            Ok("  中文\ntext\n".into())
        );
    }

    fn snapshot(primary: &str, clipboard: &str) -> Snapshot {
        Snapshot {
            primary: usable_text(Some(primary.into())),
            clipboard: usable_text(Some(clipboard.into())),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn changed_clipboard_beats_unchanged_primary_and_remains_selected() {
        let old = snapshot("old selection", "old clipboard");
        let new = snapshot("old selection", "new CLI clipboard");
        assert_eq!(
            new.select(Some(&old), Some(Source::Primary)),
            Source::Clipboard
        );
        assert_eq!(
            new.select(Some(&new), Some(Source::Clipboard)),
            Source::Clipboard
        );
        assert_eq!(new.select(Some(&old), None), Source::Clipboard);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_selection_wins_over_new_clipboard_and_failure_falls_back() {
        let old = snapshot("selected", "old clipboard");
        let new = snapshot("selected", "new clipboard");
        assert_eq!(
            new.select(Some(&old), Some(Source::Clipboard)),
            Source::Primary
        );
        for error in [
            Error::NoText,
            Error::AccessibilityRequired,
            Error::SelectionUnavailable,
        ] {
            let fallback = Snapshot {
                primary: Err(error),
                clipboard: Ok("copied".into()),
            };
            assert_eq!(
                fallback.select(Some(&old), Some(Source::Primary)),
                Source::Clipboard
            );
        }
    }

    #[test]
    fn new_primary_wins_and_unchanged_clipboard_does_not_override_it() {
        let old = snapshot("old", "copied");
        let new = snapshot("new selection", "copied");
        assert_eq!(
            new.select(Some(&old), Some(Source::Clipboard)),
            Source::Primary
        );
        assert_eq!(
            new.select(Some(&new), Some(Source::Primary)),
            Source::Primary
        );
        let both = snapshot("another selection", "another clipboard");
        assert_eq!(
            both.select(Some(&new), Some(Source::Clipboard)),
            Source::Primary
        );
    }

    #[test]
    fn first_unchanged_capture_prefers_primary_and_missing_sources_fall_back() {
        let same = snapshot("selection", "clipboard");
        assert_eq!(same.select(Some(&same), None), Source::Primary);
        assert_eq!(same.select(None, None), Source::Primary);
        let no_primary = snapshot("", "clipboard");
        assert_eq!(
            no_primary.select(Some(&same), Some(Source::Primary)),
            Source::Clipboard
        );
        let no_clipboard = snapshot("selection", "");
        assert_eq!(
            no_clipboard.select(Some(&same), Some(Source::Clipboard)),
            Source::Primary
        );
        let empty = snapshot("", "");
        assert_eq!(
            empty.text(empty.select(Some(&same), Some(Source::Primary))),
            Err(Error::NoText)
        );
    }

    #[test]
    fn debug_output_contains_sizes_but_never_the_captured_text() {
        let text = snapshot("PRIVATE_PRIMARY", "PRIVATE_CLIPBOARD");
        let log = format!("{text:?}");
        assert!(!log.contains("PRIVATE"));
        assert!(log.contains("primary_bytes"));
        assert!(log.contains("clipboard_bytes"));
    }
}
