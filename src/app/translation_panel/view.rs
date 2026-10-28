use iced::widget::{Space, button, column, container, row, scrollable, text_editor};
use iced::{Alignment, Element, Fill};
use rust_i18n::t;

use super::{Message, ServiceState, TranslationPanel};
use crate::i18n::{Langs, Language};
use crate::theme::{
    styles,
    tokens::{color, spacing, typography},
};
use crate::widgets::{editor, hint, label};

pub(super) fn source_key_binding(
    key_press: text_editor::KeyPress,
) -> Option<text_editor::Binding<Message>> {
    let submit = key_press.modifiers.is_empty();
    // 先沿用编辑器的默认绑定，让焦点检查和其他编辑快捷键保持原有行为。
    match text_editor::Binding::from_key_press(key_press) {
        Some(text_editor::Binding::Enter) if submit => {
            Some(text_editor::Binding::Custom(Message::Translate))
        }
        binding => binding,
    }
}

impl TranslationPanel {
    pub fn view(&self, language: Language) -> Element<'_, Message> {
        let locale = language.locale();

        column![
            // 两个编辑器共享剩余高度，长原文不会把译文推出窗口。
            row![self.source_editor(locale), self.translation_output(locale)]
                .spacing(spacing::PANEL_GAP)
                .height(Fill),
            self.footer(locale),
        ]
        .spacing(spacing::SECTION_GAP)
        .height(Fill)
        .into()
    }

    pub fn direction(&self, language: Language) -> Element<'_, Message> {
        let locale = language.locale();
        let (source, target) = match self.source_language {
            Language::Chinese => (Langs::CHINESE, Langs::ENGLISH),
            Language::English => (Langs::ENGLISH, Langs::CHINESE),
        };

        row![
            label(t!(source, locale = locale)).size(typography::BODY_SIZE),
            hint(
                button(label("⇄").size(typography::ICON_SIZE))
                    .on_press(Message::SwapLanguages)
                    .style(styles::quiet_button)
                    .padding(spacing::TEXT_BUTTON_PADDING),
                t!(Langs::SWAP_LANGUAGES, locale = locale),
            ),
            label(t!(target, locale = locale)).size(typography::BODY_SIZE),
        ]
        .spacing(spacing::DIRECTION_GAP)
        .align_y(Alignment::Center)
        .into()
    }

    fn source_editor(&self, locale: &str) -> Element<'_, Message> {
        container(
            column![
                row![
                    label(t!(Langs::SOURCE_TEXT, locale = locale))
                        .size(typography::ACTION_SIZE)
                        .color(color::TEXT_MUTED),
                    Space::new().width(Fill),
                    button(label(t!(Langs::CLEAR, locale = locale)).size(typography::ACTION_SIZE))
                        .on_press_maybe((!self.input.is_empty()).then_some(Message::Clear))
                        .style(styles::quiet_button)
                        .padding(spacing::TEXT_BUTTON_PADDING),
                ]
                .align_y(Alignment::Center),
                editor(
                    text_editor(&self.input)
                        .id(super::INPUT_ID)
                        .placeholder(t!(Langs::SOURCE_PLACEHOLDER, locale = locale))
                        .on_action(Message::InputAction)
                        .key_binding(source_key_binding),
                ),
            ]
            .spacing(spacing::PANEL_GAP)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .padding(spacing::PANEL_PADDING)
        .style(styles::panel)
        .into()
    }

    fn translation_output(&self, locale: &str) -> Element<'_, Message> {
        container(
            column![
                row![
                    label(t!(Langs::TRANSLATED_TEXT, locale = locale))
                        .size(typography::ACTION_SIZE)
                        .color(color::TEXT_MUTED),
                    Space::new().width(Fill),
                    button(label(t!(Langs::COPY, locale = locale)).size(typography::ACTION_SIZE))
                        .on_press_maybe((!self.output.is_empty()).then_some(Message::Copy))
                        .style(styles::quiet_button)
                        .padding(spacing::TEXT_BUTTON_PADDING),
                ]
                .align_y(Alignment::Center),
                editor(text_editor(&self.output).on_action(Message::OutputAction)),
            ]
            .spacing(spacing::PANEL_GAP)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .padding(spacing::PANEL_PADDING)
        .style(styles::panel)
        .into()
    }

    fn footer(&self, locale: &str) -> Element<'_, Message> {
        let status = match &self.service {
            ServiceState::Loading => t!(Langs::TRANSLATION_LOADING, locale = locale).into_owned(),
            ServiceState::Failed(error) => error.message(locale),
            ServiceState::Ready(_) => {
                if let Some(error) = &self.error {
                    error.message(locale)
                } else {
                    String::new()
                }
            }
        };
        let has_error = matches!(self.service, ServiceState::Failed(_)) || self.error.is_some();
        let action = if self.translating {
            Langs::TRANSLATING
        } else {
            Langs::TRANSLATE
        };

        row![
            // 路径等错误信息可能很长，限制提示高度，保留两个文本区的空间。
            container(scrollable(
                label(status)
                    .size(typography::CAPTION_SIZE)
                    .color(if has_error {
                        color::DANGER
                    } else {
                        color::TEXT_MUTED
                    }),
            ))
            .max_height(48)
            .width(Fill),
            hint(
                button(label(t!(action, locale = locale)).size(typography::BODY_SIZE))
                    .on_press_maybe(self.can_translate().then_some(Message::Translate))
                    .padding(spacing::PRIMARY_BUTTON_PADDING)
                    .style(styles::primary_button),
                t!(Langs::SOURCE_KEYBOARD_HINT, locale = locale),
            ),
        ]
        .spacing(spacing::FOOTER_GAP)
        .align_y(Alignment::Center)
        .into()
    }
}
