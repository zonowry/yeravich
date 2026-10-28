pub mod styles;
pub mod tokens;

use iced::{Theme, theme::Palette};

use tokens::color;

pub fn light() -> Theme {
    Theme::custom(
        "Yeravich Light",
        Palette {
            // Iced derives control backgrounds from this surface color.
            // The page background is applied separately by styles::background.
            background: color::SURFACE,
            text: color::TEXT,
            primary: color::PRIMARY,
            success: color::SUCCESS,
            warning: color::WARNING,
            danger: color::DANGER,
        },
    )
}
