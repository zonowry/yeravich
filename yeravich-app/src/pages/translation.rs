use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Alignment, Element, Fill, Task};
use yeravich_core::Yeravich;

use crate::controllers::translation::{Completion, TranslationController};

pub struct Page {
    controller: TranslationController,
    source: text_editor::Content,
}

#[derive(Debug, Clone)]
pub enum Message {
    SourceEdited(text_editor::Action),
    SwapLanguages,
    Translate,
    Completed(Completion),
}

impl Page {
    pub fn new(core: Yeravich) -> Self {
        Self {
            controller: TranslationController::new(core),
            source: text_editor::Content::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SourceEdited(action) => {
                if !self.controller.state().busy {
                    self.source.perform(action);
                    self.controller.set_source(self.source.text());
                }
            }
            Message::SwapLanguages => self.controller.swap_languages(),
            Message::Translate => {
                if let Some(pending) = self.controller.translate() {
                    return Task::perform(pending.future, move |result| {
                        result.ok().map(|result| {
                            Message::Completed(Completion {
                                id: pending.id,
                                result,
                            })
                        })
                    })
                    .and_then(Task::done);
                }
            }
            Message::Completed(completion) => self.controller.complete(completion),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let state = self.controller.state();
        let languages = row![
            text(&state.languages.source),
            button("⇄").on_press_maybe((!state.busy).then_some(Message::SwapLanguages)),
            text(&state.languages.target),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let mut source = text_editor(&self.source)
            .placeholder("Type or capture text to translate")
            .height(Fill)
            .padding(12);
        if !state.busy {
            source = source.on_action(Message::SourceEdited);
        }

        let actions = row![
            text(&state.status).style(text::secondary).width(Fill),
            button(if state.busy {
                "Translating…"
            } else {
                "Translate"
            })
            .on_press_maybe(
                (!state.busy && !state.source_text.trim().is_empty()).then_some(Message::Translate)
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let output = if state.translated_text.is_empty() {
            text("Translation appears here").style(text::secondary)
        } else {
            text(&state.translated_text)
        };
        let result = container(scrollable(output.width(Fill)))
            .padding(16)
            .width(Fill)
            .height(Fill)
            .style(container::rounded_box);

        container(column![languages, source, actions, result].spacing(14))
            .padding(24)
            .width(Fill)
            .height(Fill)
            .into()
    }
}
