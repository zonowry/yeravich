use std::time::Duration;

use iced::widget::{self, text, tooltip};
use iced::{Element, Fill};

use crate::theme::{
    styles,
    tokens::{spacing, typography},
};

/// Use advanced shaping so localized labels can use fallback fonts.
pub fn label<'a>(content: impl text::IntoFragment<'a>) -> widget::Text<'a> {
    text(content).shaping(text::Shaping::Advanced)
}

pub fn editor<'a, H: text::Highlighter + 'a, Message: 'a>(
    editor: widget::TextEditor<'a, H, Message>,
) -> Element<'a, Message> {
    editor
        .size(typography::EDITOR_SIZE)
        .line_height(text::LineHeight::Absolute(
            typography::EDITOR_LINE_HEIGHT.into(),
        ))
        .wrapping(text::Wrapping::WordOrGlyph)
        .padding(spacing::EDITOR_PADDING)
        .height(Fill)
        .style(styles::editor)
        .into()
}

pub fn hint<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    description: impl text::IntoFragment<'a>,
) -> Element<'a, Message> {
    tooltip(
        content,
        label(description).size(typography::CAPTION_SIZE),
        tooltip::Position::FollowCursor,
    )
    .delay(Duration::from_millis(600))
    .padding(8)
    .style(styles::panel)
    .into()
}
