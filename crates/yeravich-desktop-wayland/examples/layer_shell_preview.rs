use iced::{Element, Task};
use iced_layershell::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::to_layer_message;
use yeravich_core::AppState;
use yeravich_ui::UiMessage;

#[derive(Debug, Default)]
struct Preview {
    state: AppState,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    Ui(UiMessage),
}

fn main() -> Result<(), iced_layershell::Error> {
    application(Preview::default, namespace, update, view)
        .settings(Settings {
            id: Some("com.zonowry.yeravich.preview".into()),
            layer_settings: LayerShellSettings {
                anchor: Anchor::empty(),
                layer: Layer::Overlay,
                exclusive_zone: 0,
                size: Some((460, 220)),
                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                start_mode: StartMode::Active,
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
fn update(_preview: &mut Preview, message: Message) -> Task<Message> {
    match message {
        Message::Ui(UiMessage::Close) => iced::exit(),
        _ => Task::none(),
    }
}

fn view(preview: &Preview) -> Element<'_, Message> {
    yeravich_ui::overlay_view(&preview.state).map(Message::Ui)
}
