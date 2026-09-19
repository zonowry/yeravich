use std::collections::BTreeMap;

use yeravich_core::{
    AppEvent, AppPhase, AppState, Capability, CapabilityError, Effect, ProviderProfile, RequestId,
    TranslationEvent, reduce,
};

fn configured_state() -> AppState {
    AppState::default().with_provider(ProviderProfile {
        name: "test".into(),
        adapter_id: "fake".into(),
        public: BTreeMap::new(),
        secret_ref: None,
    })
}

fn trigger_and_select() -> (AppState, RequestId) {
    let (state, effects) = reduce(configured_state(), AppEvent::Triggered);
    let id = match effects.as_slice() {
        [Effect::AcquireSelection { id }] => *id,
        other => panic!("unexpected effects: {other:?}"),
    };
    let (state, effects) = reduce(
        state,
        AppEvent::SelectionAcquired {
            id,
            text: "hello".into(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::ShowOverlay, Effect::Translate(_)]
    ));
    (state, id)
}

#[test]
fn normal_non_streaming_flow_completes() {
    let (state, id) = trigger_and_select();
    let (state, effects) = reduce(
        state,
        AppEvent::Translation(TranslationEvent::Completed {
            id,
            text: "你好".into(),
        }),
    );
    assert!(effects.is_empty());
    assert!(
        matches!(state.phase, AppPhase::Completed { translation, .. } if translation == "你好")
    );
}

#[test]
fn streaming_deltas_accumulate_until_completed() {
    let (state, id) = trigger_and_select();
    let (state, _) = reduce(
        state,
        AppEvent::Translation(TranslationEvent::Delta {
            id,
            text: "你".into(),
        }),
    );
    assert!(matches!(&state.phase, AppPhase::Translating { output, .. } if output == "你"));
    let (state, _) = reduce(
        state,
        AppEvent::Translation(TranslationEvent::Delta {
            id,
            text: "好".into(),
        }),
    );
    assert!(matches!(&state.phase, AppPhase::Translating { output, .. } if output == "你好"));
}

#[test]
fn failures_are_displayable_and_close_returns_idle() {
    let (state, effects) = reduce(configured_state(), AppEvent::Triggered);
    let Effect::AcquireSelection { id } = effects[0] else {
        unreachable!()
    };
    let error = CapabilityError::unavailable(Capability::AtSpiSelection, "not running");
    let (state, effects) = reduce(state, AppEvent::SelectionFailed { id, error });
    assert!(matches!(state.phase, AppPhase::Error { .. }));
    assert_eq!(effects, vec![Effect::ShowOverlay]);
    let (state, effects) = reduce(state, AppEvent::Closed);
    assert_eq!(state.phase, AppPhase::Idle);
    assert_eq!(effects, vec![Effect::HideOverlay]);
}

#[test]
fn duplicate_trigger_invalidates_older_results() {
    let (state, first_effects) = reduce(configured_state(), AppEvent::Triggered);
    let Effect::AcquireSelection { id: first } = first_effects[0] else {
        unreachable!()
    };
    let (state, second_effects) = reduce(state, AppEvent::Triggered);
    let Effect::AcquireSelection { id: second } = second_effects[0] else {
        unreachable!()
    };
    assert_ne!(first, second);

    let (state, effects) = reduce(
        state,
        AppEvent::SelectionAcquired {
            id: first,
            text: "stale".into(),
        },
    );
    assert!(effects.is_empty());
    assert_eq!(state.phase, AppPhase::AcquiringSelection { id: second });
}

#[test]
fn stale_translation_events_are_ignored() {
    let (state, id) = trigger_and_select();
    let outdated_request = RequestId(id.0 + 10);
    let before = state.clone();
    let (state, effects) = reduce(
        state,
        AppEvent::Translation(TranslationEvent::Completed {
            id: outdated_request,
            text: "wrong".into(),
        }),
    );
    assert_eq!(state, before);
    assert!(effects.is_empty());
}

#[test]
fn active_translation_failure_enters_error_state() {
    let (state, id) = trigger_and_select();
    let (state, effects) = reduce(
        state,
        AppEvent::TranslationFailed {
            id,
            error: "provider offline".into(),
        },
    );
    assert!(effects.is_empty());
    assert!(
        matches!(state.phase, AppPhase::Error { message, .. } if message == "provider offline")
    );
}
