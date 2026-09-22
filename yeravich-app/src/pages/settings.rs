use std::fmt;

use iced::{
    Element, Fill, Task,
    widget::{button, checkbox, column, container, row, scrollable, text, text_input},
};
use yeravich_core::settings::SettingsService;

use crate::controllers::settings::{Activity, Completion, SecretInput, SettingsController};

pub struct Page {
    controller: SettingsController,
}

#[derive(Clone)]
pub enum Message {
    AddressChanged(String),
    ModelChanged(String),
    KeyChanged(SecretInput),
    RemoveKey(bool),
    Test,
    Save,
    Completed(Completion),
}

impl fmt::Debug for Message {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Addresses may contain pasted credentials before input validation.
        formatter.write_str("SettingsMessage([redacted])")
    }
}

impl Page {
    pub fn new(service: SettingsService) -> Self {
        Self {
            controller: SettingsController::new(service),
        }
    }

    pub fn is_saving(&self) -> bool {
        self.controller.state().activity == Activity::Saving
    }

    pub fn leave(&mut self) {
        self.controller.leave();
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddressChanged(value) => self.controller.set_address(value),
            Message::ModelChanged(value) => self.controller.set_model(value),
            Message::KeyChanged(value) => self.controller.set_key(value),
            Message::RemoveKey(remove) => self.controller.remove_key(remove),
            Message::Completed(completion) => self.controller.complete(completion),
            Message::Test | Message::Save => {
                let activity = if matches!(message, Message::Test) {
                    Activity::Testing
                } else {
                    Activity::Saving
                };
                if let Some(pending) = self.controller.start(activity) {
                    return Task::perform(pending.future, move |result| {
                        result.ok().map(|outcome| {
                            Message::Completed(Completion {
                                id: pending.id,
                                outcome,
                            })
                        })
                    })
                    .and_then(Task::done);
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let state = self.controller.state();
        let editable = state.activity != Activity::Saving;
        let address = text_input("https://api.example.com/v1", &state.address)
            .on_input_maybe(editable.then_some(Message::AddressChanged))
            .padding(10);
        let model = text_input("Model ID provided by your service", &state.model)
            .on_input_maybe(editable.then_some(Message::ModelChanged))
            .padding(10);
        let key_hint = if state.has_saved_key && !state.remove_key {
            "Saved key — leave blank to keep it"
        } else {
            "Optional for services without authentication"
        };
        let key = text_input(key_hint, state.key.as_str())
            .secure(true)
            .on_input_maybe(
                editable.then_some(|value| Message::KeyChanged(SecretInput::new(value))),
            )
            .padding(10);
        let mut form = column![
            text("OpenAI-compatible service").size(22),
            text("API address"),
            address,
            text("Use a base URL (including /v1) or the full /chat/completions URL.")
                .size(13)
                .style(text::secondary),
            text("API key"),
            key,
        ]
        .spacing(10);
        if state.has_saved_key {
            form = form.push(
                checkbox(state.remove_key)
                    .label("Remove the saved API key on save")
                    .on_toggle_maybe(editable.then_some(Message::RemoveKey)),
            );
        }
        form = form.push(text("Model")).push(model)
            .push(text("Test sends a short request using these fields and may use provider quota. It does not save changes.").size(13).style(text::secondary))
            .push(row![
                button(if state.activity == Activity::Testing { "Test again" } else { "Test connection" }).on_press_maybe(editable.then_some(Message::Test)).style(button::secondary),
                button(if editable { "Save" } else { "Saving…" }).on_press_maybe(editable.then_some(Message::Save)),
            ].spacing(10))
            .push(text(&state.status).style(if state.error { text::danger } else { text::secondary }));
        container(scrollable(form))
            .padding(24)
            .width(Fill)
            .height(Fill)
            .into()
    }
}
