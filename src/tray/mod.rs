//! 托盘只发送意图；显示窗口、保留文本和退出仍由 App 决定。
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native;

#[cfg(target_os = "linux")]
pub use linux::Registration;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use native::Registration;

use rust_i18n::t;

use crate::i18n::{Langs, Language};

// 从现有 SVG 导出的 32×32 RGBA，编译进程序，不依赖运行目录或图片解码库。
const ICON_SIZE: u32 = 32;
const ICON_RGBA: &[u8; 32 * 32 * 4] = include_bytes!("../../data/tray-icon.rgba");

#[derive(Debug, Clone)]
pub enum Event {
    Ready,
    Unavailable(String),
    Show,
    Exit,
    Stopped,
}

struct Labels {
    show: String,
    exit: String,
}

impl Labels {
    fn new(language: Language) -> Self {
        Self {
            show: t!(Langs::SHOW_WINDOW, locale = language.locale()).into_owned(),
            exit: t!(Langs::EXIT, locale = language.locale()).into_owned(),
        }
    }
}
