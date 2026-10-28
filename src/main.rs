mod app;
mod config;
mod i18n;
mod shortcut;
mod text_capture;
mod theme;
mod translation;
mod tray;
mod widgets;

rust_i18n::i18n!("locales", fallback = "en");

use app::App;
use theme::tokens::typography;

fn main() -> iced::Result {
    iced::daemon(App::boot, App::update, App::view)
        .title(|app: &App, _window| app.title())
        .theme(theme::light())
        .default_font(typography::DEFAULT_FONT)
        .subscription(App::subscription)
        .run()
}
