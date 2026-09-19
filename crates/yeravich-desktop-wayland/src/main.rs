use iced::Element;
use yeravich_desktop_wayland::{DesktopApp, Message, update};

fn main() -> iced::Result {
    iced::application(DesktopApp::default, update, view)
        .title("Yeravich Settings")
        .window_size((560.0, 380.0))
        .run()
}

fn view(app: &DesktopApp) -> Element<'_, Message> {
    yeravich_ui::settings_view(&app.state).map(Message::Ui)
}
