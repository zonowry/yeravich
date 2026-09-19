use iced::{Element, Task, window};
use iced_layershell::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::to_layer_message;
use yeravich_core::AppState;
use yeravich_ui::UiMessage;

#[derive(Debug, Default)]
struct DaemonState {
    app: AppState,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
enum Message {
    Ui(UiMessage),
}

fn main() -> Result<(), iced_layershell::Error> {
    daemon(DaemonState::default, namespace, update, view)
        .settings(Settings {
            id: Some("com.zonowry.yeravich".into()),
            layer_settings: LayerShellSettings {
                start_mode: StartMode::Background,
                ..LayerShellSettings::default()
            },
            ..Settings::default()
        })
        .run()
}

fn namespace() -> String {
    "com.zonowry.yeravich".into()
}

#[allow(clippy::needless_pass_by_value)]
fn update(_state: &mut DaemonState, message: Message) -> Task<Message> {
    if matches!(message, Message::Ui(UiMessage::Close)) {
        return iced::exit();
    }
    Task::none()
}

fn view(state: &DaemonState, _window: window::Id) -> Element<'_, Message> {
    yeravich_ui::overlay_view(&state.app).map(Message::Ui)
}
