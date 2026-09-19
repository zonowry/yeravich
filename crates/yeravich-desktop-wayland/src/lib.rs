//! Wayland application assembly and short-lived effect interpretation.

use iced::Task;
use yeravich_core::{AppEvent, AppState, Capability, CapabilityError, Effect, RequestId, reduce};
use yeravich_ui::UiMessage;

#[derive(Debug, Clone)]
pub enum Message {
    Ui(UiMessage),
    Core(AppEvent),
}

#[derive(Debug, Default)]
pub struct DesktopApp {
    pub state: AppState,
}

pub fn update(app: &mut DesktopApp, message: Message) -> Task<Message> {
    let event = match message {
        Message::Ui(UiMessage::Trigger) => AppEvent::Triggered,
        Message::Ui(UiMessage::Close) => AppEvent::Closed,
        Message::Core(event) => event,
    };
    let (next, effects) = reduce(std::mem::take(&mut app.state), event);
    app.state = next;
    Task::batch(effects.into_iter().map(interpret))
}

/// Converts short-lived domain effects into runtime tasks. Real adapters will
/// replace the explicit capability failures in later implementation phases.
pub fn interpret(effect: Effect) -> Task<Message> {
    match effect {
        Effect::AcquireSelection { id } => capability_failure(
            id,
            Capability::AtSpiSelection,
            "selection adapters are not connected in this scaffold",
        ),
        Effect::Translate(request) => Task::done(Message::Core(AppEvent::TranslationFailed {
            id: request.id,
            error: "translation providers are not connected in this scaffold".into(),
        })),
        Effect::ShowOverlay | Effect::HideOverlay => Task::none(),
    }
}

fn capability_failure(id: RequestId, capability: Capability, message: &str) -> Task<Message> {
    Task::done(Message::Core(AppEvent::SelectionFailed {
        id,
        error: CapabilityError::unavailable(capability, message),
    }))
}
