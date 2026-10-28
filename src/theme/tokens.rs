//! Shared visual values. Views and widget styles refer to these semantic tokens.

pub mod color {
    use iced::Color;

    pub const BACKGROUND: Color = Color::from_rgb8(247, 248, 250);
    pub const SURFACE: Color = Color::WHITE;
    pub const TEXT: Color = Color::from_rgb8(35, 40, 48);
    pub const TEXT_MUTED: Color = Color::from_rgb8(106, 116, 131);
    pub const BORDER: Color = Color::from_rgb8(224, 230, 237);
    pub const HOVER: Color = Color::from_rgb8(235, 238, 242);
    pub const DISABLED: Color = Color::from_rgb8(163, 170, 180);
    pub const SELECTION: Color = Color::from_rgb8(213, 227, 248);

    pub const PRIMARY: Color = Color::from_rgb8(48, 60, 77);
    pub const SUCCESS: Color = Color::from_rgb8(18, 102, 79);
    pub const WARNING: Color = Color::from_rgb8(183, 126, 51);
    pub const DANGER: Color = Color::from_rgb8(195, 66, 63);
}

pub mod typography {
    use iced::Font;

    pub const DEFAULT_FONT: Font = Font::DEFAULT;
    pub const BODY_SIZE: f32 = 15.0;
    pub const CAPTION_SIZE: f32 = 12.0;
    pub const ACTION_SIZE: f32 = 13.0;
    pub const EDITOR_SIZE: f32 = 17.0;
    pub const EDITOR_LINE_HEIGHT: f32 = 24.0;
    pub const ICON_SIZE: f32 = 18.0;
}

pub mod spacing {
    pub const PAGE_PADDING: f32 = 12.0;
    pub const SECTION_GAP: f32 = 10.0;
    pub const HEADER_GAP: f32 = 8.0;
    pub const DIRECTION_GAP: f32 = 4.0;
    pub const FOOTER_GAP: f32 = 12.0;
    pub const PANEL_PADDING: f32 = 12.0;
    pub const PANEL_GAP: f32 = 10.0;
    pub const EDITOR_PADDING: f32 = 2.0;

    // Two-value padding is [vertical, horizontal].
    pub const PICKER_PADDING: [f32; 2] = [5.0, 8.0];
    pub const TEXT_BUTTON_PADDING: [f32; 2] = [4.0, 8.0];
    pub const PRIMARY_BUTTON_PADDING: [f32; 2] = [7.0, 22.0];
}

pub mod border {
    pub const WIDTH: f32 = 1.0;
    pub const PANEL_RADIUS: f32 = 6.0;
    pub const CONTROL_RADIUS: f32 = 4.0;
}

pub mod layout {
    use iced::Size;

    pub const WINDOW_SIZE: Size = Size::new(800.0, 420.0);
    pub const MIN_WINDOW_SIZE: Size = Size::new(620.0, 320.0);
}
