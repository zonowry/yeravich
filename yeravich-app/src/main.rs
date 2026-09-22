mod app;
mod controllers;
mod pages;
mod platform;
mod task_scope;

use app::App;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Yeravich")
        .window(iced::window::Settings {
            size: iced::Size::new(620.0, 520.0),
            min_size: Some(iced::Size::new(420.0, 400.0)),
            platform_specific: iced::window::settings::PlatformSpecific {
                #[cfg(target_os = "linux")]
                application_id: "com.zonowry.yeravich".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}
