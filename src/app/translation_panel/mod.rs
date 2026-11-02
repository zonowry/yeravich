mod update;
mod view;

#[cfg(test)]
mod tests;

use iced::widget::text_editor;

use crate::i18n::Language;
use crate::translation;

pub(super) const INPUT_ID: &str = "source-text";

pub(super) struct TranslationPanel {
    service: ServiceState,
    source_language: Language,
    input: text_editor::Content,
    output: text_editor::Content,
    revision: u64,
    translating: bool,
    translation_task: Option<iced::task::Handle>,
    auto_translate_pending: bool,
    error: Option<translation::Error>,
}

enum ServiceState {
    Loading,
    Ready(translation::Service),
    Failed(translation::Error),
}

#[derive(Debug, Clone)]
pub enum Message {
    ServiceLoaded(Result<translation::Service, translation::Error>),
    InputAction(text_editor::Action),
    OutputAction(text_editor::Action),
    SwapLanguages,
    Clear,
    Copy,
    Translate,
    TranslationDelta {
        revision: u64,
        delta: String,
    },
    Translated {
        revision: u64,
        result: Result<(), translation::Error>,
    },
}

impl TranslationPanel {
    pub fn new(source_language: Language) -> Self {
        Self {
            service: ServiceState::Loading,
            source_language,
            input: text_editor::Content::new(),
            output: text_editor::Content::new(),
            revision: 0,
            translating: false,
            translation_task: None,
            auto_translate_pending: false,
            error: None,
        }
    }

    fn can_translate(&self) -> bool {
        matches!(self.service, ServiceState::Ready(_))
            && !self.translating
            && !self.input.text().trim().is_empty()
    }

    fn import_text(&mut self, text: &str) {
        if !text.trim().is_empty() && self.input.text() != text {
            self.input = text_editor::Content::with_text(text);
            self.invalidate_translation();
        }
    }

    fn invalidate_translation(&mut self) {
        // 释放 abort_on_drop 句柄会取消网络读取；已排队的消息仍需检查 revision。
        self.translation_task = None;
        self.revision += 1;
        self.output = text_editor::Content::new();
        self.translating = false;
        self.auto_translate_pending = false;
        self.error = None;
    }
}
