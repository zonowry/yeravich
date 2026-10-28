use iced::widget::{button, container, pick_list, text_editor};
use iced::{Border, Theme};

use super::tokens::{border, color};

pub fn background(_theme: &Theme) -> container::Style {
    container::Style::default().background(color::BACKGROUND)
}

pub fn panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(color::SURFACE)
        .border(Border {
            color: color::BORDER,
            width: border::WIDTH,
            radius: border::PANEL_RADIUS.into(),
        })
}

pub fn editor(_theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: color::SURFACE.into(),
        // 面板已经提供边界，不再在文本周围套第二层输入框。
        border: Border::default(),
        placeholder: color::TEXT_MUTED,
        value: color::TEXT,
        selection: color::SELECTION,
    }
}

pub fn quiet_button(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(color::HOVER.into()),
        text_color: if matches!(status, button::Status::Disabled) {
            color::DISABLED
        } else {
            color::TEXT_MUTED
        },
        border: Border {
            radius: border::CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Disabled => (color::HOVER, color::TEXT_MUTED),
        button::Status::Hovered | button::Status::Pressed => (color::TEXT, color::SURFACE),
        button::Status::Active => (color::PRIMARY, color::SURFACE),
    };
    button::Style {
        background: Some(background.into()),
        text_color,
        border: Border {
            radius: border::CONTROL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn language_picker(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: color::TEXT_MUTED,
        placeholder_color: color::TEXT_MUTED,
        handle_color: color::TEXT_MUTED,
        background: match status {
            pick_list::Status::Active => color::BACKGROUND,
            _ => color::HOVER,
        }
        .into(),
        border: Border {
            radius: border::CONTROL_RADIUS.into(),
            ..Border::default()
        },
    }
}
