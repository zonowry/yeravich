//! Declarative, platform-independent Yeravich views.

use iced::widget::{button, column, container, row, rule, text};
use iced::{Alignment, Element, Length};
use yeravich_core::{AppPhase, AppState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMessage {
    Trigger,
    Close,
}

#[must_use]
pub fn settings_view(state: &AppState) -> Element<'_, UiMessage> {
    let provider = state
        .provider
        .as_ref()
        .map_or("Not configured", |profile| profile.name.as_str());

    let content = column![
        text("Yeravich").size(30),
        text("Desktop translation overlay scaffold"),
        rule::horizontal(1),
        row![
            text("Language pair"),
            text(format!(
                "{} → {}",
                state.languages.source, state.languages.target
            ))
        ]
        .spacing(16),
        row![text("Provider"), text(provider)].spacing(16),
        text(phase_label(&state.phase)),
        button("Test activation").on_press(UiMessage::Trigger),
    ]
    .spacing(14)
    .padding(24)
    .width(Length::Fill);

    container(content)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

#[must_use]
pub fn overlay_view(state: &AppState) -> Element<'_, UiMessage> {
    let (heading, body) = match &state.phase {
        AppPhase::Idle => (
            "Yeravich preview",
            "Layer-shell overlay is ready.".to_owned(),
        ),
        AppPhase::AcquiringSelection { .. } => ("Reading selection", "Please wait…".to_owned()),
        AppPhase::Translating { output, .. } => ("Translating", output.clone()),
        AppPhase::Completed { translation, .. } => ("Translation", translation.clone()),
        AppPhase::Error { message, .. } => ("Capability unavailable", message.clone()),
    };

    container(
        column![
            text(heading).size(24),
            text(body),
            button("Close").on_press(UiMessage::Close),
        ]
        .align_x(Alignment::Center)
        .spacing(16)
        .padding(24),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn phase_label(phase: &AppPhase) -> String {
    match phase {
        AppPhase::Idle => "Status: idle".into(),
        AppPhase::AcquiringSelection { .. } => "Status: acquiring selection".into(),
        AppPhase::Translating { .. } => "Status: translating".into(),
        AppPhase::Completed { .. } => "Status: completed".into(),
        AppPhase::Error { message, .. } => format!("Status: {message}"),
    }
}
