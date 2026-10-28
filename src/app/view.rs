use iced::widget::{Space, button, column, container, pick_list, row, text};
use iced::{Alignment, Element, Fill};
use rust_i18n::t;

use super::{App, Message, ShortcutStatus};
use crate::i18n::{Langs, Language};
use crate::theme::{
    styles,
    tokens::{color, spacing, typography},
};
use crate::widgets::{hint, label};

impl App {
    pub fn view(&self, _window: iced::window::Id) -> Element<'_, Message> {
        let locale = self.language.locale();
        let notice = if self.close_blocked {
            Some(t!(Langs::CLOSE_BLOCKED, locale = locale))
        } else if matches!(self.shortcut_status, ShortcutStatus::Failed) {
            Some(t!(Langs::SHORTCUT_UNAVAILABLE, locale = locale))
        } else if self.tray_failed {
            Some(t!(Langs::TRAY_UNAVAILABLE, locale = locale))
        } else {
            None
        };
        let mut content = column![
            self.header(),
            self.panel.view(self.language).map(Message::Panel),
        ]
        .spacing(spacing::SECTION_GAP)
        .height(Fill);

        if let Some(notice) = notice {
            content = content.push(
                label(notice)
                    .size(typography::CAPTION_SIZE)
                    .color(color::DANGER),
            );
        } else if let Some(error) = self.capture_notice {
            content = content.push(
                label(error.message(self.language))
                    .size(typography::CAPTION_SIZE)
                    .color(color::DANGER),
            );
        }

        container(content)
            .padding(spacing::PAGE_PADDING)
            .width(Fill)
            .height(Fill)
            .style(styles::background)
            .into()
    }

    fn header(&self) -> Element<'_, Message> {
        let locale = self.language.locale();
        let exit_hint = match &self.shortcut_status {
            ShortcutStatus::Ready(shortcut) if !shortcut.is_empty() => {
                t!(
                    Langs::EXIT_HINT_SHORTCUT,
                    locale = locale,
                    shortcut = shortcut
                )
            }
            _ => t!(Langs::EXIT_HINT, locale = locale),
        };

        let header = row![
            self.panel.direction(self.language).map(Message::Panel),
            Space::new().width(Fill),
        ];
        #[cfg(target_os = "macos")]
        let header = header.push(hint(
            button(
                label(t!(Langs::SELECTION_PERMISSION, locale = locale))
                    .size(typography::ACTION_SIZE),
            )
            .on_press(Message::RequestSelectionPermission)
            .padding(spacing::TEXT_BUTTON_PADDING)
            .style(styles::quiet_button),
            t!(Langs::SELECTION_PERMISSION_HINT, locale = locale),
        ));
        header
            .push(hint(
                pick_list(Language::ALL, Some(self.language), Message::LanguageChanged)
                    .text_shaping(text::Shaping::Advanced)
                    .text_size(typography::ACTION_SIZE)
                    .padding(spacing::PICKER_PADDING)
                    .style(styles::language_picker),
                t!(Langs::INTERFACE_LANGUAGE, locale = locale),
            ))
            .push(hint(
                button(label(t!(Langs::EXIT, locale = locale)).size(typography::ACTION_SIZE))
                    .on_press(Message::Exit)
                    .padding(spacing::TEXT_BUTTON_PADDING)
                    .style(styles::quiet_button),
                exit_hint,
            ))
            .spacing(spacing::HEADER_GAP)
            .align_y(Alignment::Center)
            .into()
    }
}
