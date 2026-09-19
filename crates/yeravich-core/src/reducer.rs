use std::collections::BTreeMap;

use crate::{
    Capability, CapabilityError, CapabilityState, LanguagePair, ProviderProfile, RequestId,
    TranslationEvent, TranslationRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppPhase {
    Idle,
    AcquiringSelection {
        id: RequestId,
    },
    Translating {
        id: RequestId,
        source: String,
        output: String,
    },
    Completed {
        id: RequestId,
        source: String,
        translation: String,
    },
    Error {
        id: RequestId,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub phase: AppPhase,
    pub languages: LanguagePair,
    pub provider: Option<ProviderProfile>,
    pub capabilities: BTreeMap<Capability, CapabilityState>,
    next_request_id: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            phase: AppPhase::Idle,
            languages: LanguagePair::default(),
            provider: None,
            capabilities: BTreeMap::new(),
            next_request_id: 1,
        }
    }
}

impl AppState {
    #[must_use]
    pub fn with_provider(mut self, provider: ProviderProfile) -> Self {
        self.provider = Some(provider);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    Triggered,
    SelectionAcquired {
        id: RequestId,
        text: String,
    },
    SelectionFailed {
        id: RequestId,
        error: CapabilityError,
    },
    Translation(TranslationEvent),
    TranslationFailed {
        id: RequestId,
        error: String,
    },
    CapabilityChanged {
        capability: Capability,
        state: CapabilityState,
    },
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    AcquireSelection { id: RequestId },
    Translate(TranslationRequest),
    ShowOverlay,
    HideOverlay,
}

/// Applies one event without performing I/O and returns the next state plus effects.
#[must_use]
pub fn reduce(mut state: AppState, event: AppEvent) -> (AppState, Vec<Effect>) {
    match event {
        AppEvent::Triggered => {
            let id = RequestId(state.next_request_id);
            state.next_request_id = state.next_request_id.saturating_add(1);
            state.phase = AppPhase::AcquiringSelection { id };
            (state, vec![Effect::AcquireSelection { id }])
        }
        AppEvent::SelectionAcquired { id, text } if matches!(state.phase, AppPhase::AcquiringSelection { id: active } if active == id) =>
        {
            let Some(provider) = state.provider.clone() else {
                state.phase = AppPhase::Error {
                    id,
                    message: "no translation provider is configured".into(),
                };
                return (state, vec![Effect::ShowOverlay]);
            };
            state.phase = AppPhase::Translating {
                id,
                source: text.clone(),
                output: String::new(),
            };
            let languages = state.languages.clone();
            (
                state,
                vec![
                    Effect::ShowOverlay,
                    Effect::Translate(TranslationRequest {
                        id,
                        text,
                        languages,
                        provider,
                    }),
                ],
            )
        }
        AppEvent::SelectionFailed { id, error } if matches!(state.phase, AppPhase::AcquiringSelection { id: active } if active == id) =>
        {
            state.phase = AppPhase::Error {
                id,
                message: error.to_string(),
            };
            (state, vec![Effect::ShowOverlay])
        }
        AppEvent::Translation(TranslationEvent::Delta { id, text }) => {
            if let AppPhase::Translating {
                id: active, output, ..
            } = &mut state.phase
                && *active == id
            {
                output.push_str(&text);
            }
            (state, Vec::new())
        }
        AppEvent::Translation(TranslationEvent::Completed { id, text }) => {
            if let AppPhase::Translating {
                id: active, source, ..
            } = &state.phase
                && *active == id
            {
                state.phase = AppPhase::Completed {
                    id,
                    source: source.clone(),
                    translation: text,
                };
            }
            (state, Vec::new())
        }
        AppEvent::TranslationFailed { id, error } => {
            if matches!(state.phase, AppPhase::Translating { id: active, .. } if active == id) {
                state.phase = AppPhase::Error { id, message: error };
            }
            (state, Vec::new())
        }
        AppEvent::CapabilityChanged {
            capability,
            state: capability_state,
        } => {
            state.capabilities.insert(capability, capability_state);
            (state, Vec::new())
        }
        AppEvent::Closed => {
            state.phase = AppPhase::Idle;
            (state, vec![Effect::HideOverlay])
        }
        AppEvent::SelectionAcquired { .. } | AppEvent::SelectionFailed { .. } => {
            (state, Vec::new())
        }
    }
}
