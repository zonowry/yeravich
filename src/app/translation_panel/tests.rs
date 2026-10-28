use super::*;
use crate::app::{App, Message as AppMessage};
use iced::keyboard::{Key, Modifiers, key};

fn enter_key_press(modifiers: Modifiers) -> text_editor::KeyPress {
    text_editor::KeyPress {
        key: Key::Named(key::Named::Enter),
        modified_key: Key::Named(key::Named::Enter),
        physical_key: key::Physical::Code(key::Code::Enter),
        modifiers,
        text: Some("\r".into()),
        status: text_editor::Status::Focused { is_hovered: false },
    }
}

#[test]
fn enter_submits_without_editing_or_repeating_a_pending_request() {
    for code in [key::Code::Enter, key::Code::NumpadEnter] {
        let mut key_press = enter_key_press(Modifiers::empty());
        key_press.physical_key = key::Physical::Code(code);
        let Some(text_editor::Binding::Custom(message)) = view::source_key_binding(key_press)
        else {
            panic!("Enter should submit a translation");
        };
        let mut panel = ready_panel();
        panel.input = text_editor::Content::with_text("Hello\nWorld");
        let _ = panel.update(message.clone());
        assert!(panel.translating);
        assert_eq!(panel.input.text(), "Hello\nWorld");

        let revision = panel.revision;
        let _ = panel.update(message);
        assert_eq!(panel.revision, revision);
        let _ = panel.update(Message::Translated {
            revision,
            result: Ok("你好\n世界".into()),
        });
        assert_eq!(panel.output.text(), "你好\n世界");
    }
}

#[test]
fn enter_does_not_submit_empty_input_or_without_a_ready_service() {
    for (mut panel, input) in [
        (TranslationPanel::new(Language::English), "Hello"),
        (ready_panel(), ""),
        (ready_panel(), " \n "),
    ] {
        panel.input = text_editor::Content::with_text(input);
        let Some(text_editor::Binding::Custom(message)) =
            view::source_key_binding(enter_key_press(Modifiers::empty()))
        else {
            panic!("Enter should use the translation message");
        };
        let _ = panel.update(message);
        assert!(!panel.translating);
        assert_eq!(panel.revision, 0);
        assert_eq!(panel.input.text(), input);
    }
}

#[test]
fn enter_only_submits_when_the_source_editor_is_focused() {
    for status in [
        text_editor::Status::Active,
        text_editor::Status::Hovered,
        text_editor::Status::Disabled,
    ] {
        for modifiers in [Modifiers::empty(), Modifiers::SHIFT] {
            let mut key_press = enter_key_press(modifiers);
            key_press.status = status;
            assert!(view::source_key_binding(key_press).is_none());
        }
    }
}

#[test]
fn shift_enter_and_editing_shortcuts_keep_the_default_bindings() {
    assert!(matches!(
        view::source_key_binding(enter_key_press(Modifiers::SHIFT)),
        Some(text_editor::Binding::Enter)
    ));
    let binding = |key: Key, modifiers| {
        view::source_key_binding(text_editor::KeyPress {
            key: key.clone(),
            modified_key: key,
            text: None,
            ..enter_key_press(modifiers)
        })
    };
    assert!(matches!(
        binding(Key::Character("c".into()), Modifiers::COMMAND),
        Some(text_editor::Binding::Copy)
    ));
    assert!(matches!(
        binding(Key::Character("v".into()), Modifiers::COMMAND),
        Some(text_editor::Binding::Paste)
    ));
    assert!(matches!(
        binding(Key::Named(key::Named::ArrowLeft), Modifiers::empty()),
        Some(text_editor::Binding::Move(text_editor::Motion::Left))
    ));
}

fn ready_panel() -> TranslationPanel {
    let mut panel = TranslationPanel::new(Language::English);
    let service = translation::Service::new(crate::config::TranslationConfig {
        endpoint: "http://127.0.0.1:1/v1/chat/completions".parse().unwrap(),
        model: "test-model".into(),
        authorization: None,
    })
    .unwrap();
    let _ = panel.update(Message::ServiceLoaded(Ok(service)));
    panel
}

#[test]
fn imported_text_invalidates_old_translations_without_submitting_a_request() {
    let mut panel = ready_panel();
    panel.input = text_editor::Content::with_text("Old source");
    let _ = panel.update(Message::Translate);
    let old_revision = panel.revision;
    panel.import_text("  新原文\n第二行\n");
    assert_eq!(panel.input.text(), "  新原文\n第二行\n");
    assert_eq!(panel.source_language, Language::English);
    assert!(!panel.translating);
    for result in [
        Ok("Old translation".into()),
        Err(translation::Error::Network),
    ] {
        let _ = panel.update(Message::Translated {
            revision: old_revision,
            result,
        });
        assert!(panel.output.is_empty());
        assert!(panel.error.is_none());
    }
}

#[test]
fn empty_or_identical_imports_preserve_current_translation() {
    let mut panel = ready_panel();
    panel.input = text_editor::Content::with_text("Hello");
    panel.output = text_editor::Content::with_text("你好");
    for text in ["", " \n\t", "Hello"] {
        panel.import_text(text);
        assert_eq!(panel.input.text(), "Hello");
        assert_eq!(panel.output.text(), "你好");
        assert_eq!(panel.revision, 0);
    }
}

fn start_capture(app: &mut App) -> u64 {
    app.focused = false;
    let _ = app.update(AppMessage::Shortcut(crate::shortcut::Event::Activated));
    app.capture.as_ref().unwrap().revision
}

#[test]
fn capture_translates_automatically_and_rejects_duplicate_or_superseded_messages() {
    let mut app = App {
        panel: ready_panel(),
        ..App::default()
    };
    let first = start_capture(&mut app);
    let second = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision: first,
        snapshot: crate::text_capture::Snapshot {
            primary: Ok("stale".into()),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert!(app.panel.input.is_empty());
    assert!(!app.panel.translating);
    let _ = app.update(AppMessage::TextCaptured {
        revision: second,
        snapshot: crate::text_capture::Snapshot {
            primary: Ok("selected".into()),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert_eq!(app.panel.input.text(), "selected");
    assert!(app.panel.translating);
    let translation_revision = app.panel.revision;
    let _ = app.update(AppMessage::TextCaptured {
        revision: second,
        snapshot: crate::text_capture::Snapshot {
            primary: Ok("duplicate".into()),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert_eq!(app.panel.input.text(), "selected");
    assert_eq!(app.panel.revision, translation_revision);
}

#[test]
fn automatic_translation_skips_empty_input_and_reuses_pending_or_completed_results() {
    let mut panel = ready_panel();
    panel.import_text("manual input");
    for text in ["", " \n\t"] {
        let _ = panel.import_and_translate(text);
        assert_eq!(panel.input.text(), "manual input");
        assert!(!panel.translating);
    }
    let _ = panel.import_and_translate("captured");
    assert!(panel.translating);
    let revision = panel.revision;
    let _ = panel.import_and_translate("captured");
    assert_eq!(panel.revision, revision);
    let _ = panel.update(Message::Translated {
        revision,
        result: Ok("译文".into()),
    });
    let _ = panel.import_and_translate("captured");
    assert_eq!(panel.output.text(), "译文");
    assert_eq!(panel.revision, revision);
    assert!(!panel.translating);
}

#[test]
fn new_captured_text_replaces_a_request_and_failed_translations_can_be_retried() {
    let mut panel = ready_panel();
    let _ = panel.import_and_translate("first");
    let first = panel.revision;
    let _ = panel.import_and_translate("second");
    let second = panel.revision;
    assert_ne!(first, second);
    for result in [Ok("old".into()), Err(translation::Error::Network)] {
        let _ = panel.update(Message::Translated {
            revision: first,
            result,
        });
        assert!(panel.translating);
        assert!(panel.output.is_empty());
        assert!(panel.error.is_none());
    }
    let _ = panel.update(Message::Translated {
        revision: second,
        result: Err(translation::Error::Network),
    });
    let _ = panel.import_and_translate("second");
    assert!(panel.translating);
    assert!(panel.error.is_none());
    assert_ne!(panel.revision, second);
}

fn service_loaded() -> Message {
    let ServiceState::Ready(service) = ready_panel().service else {
        panic!("the test service must be ready");
    };
    Message::ServiceLoaded(Ok(service))
}

#[test]
fn captured_text_waits_for_configuration_but_manual_input_does_not_auto_submit() {
    let mut panel = TranslationPanel::new(Language::English);
    let _ = panel.import_and_translate("captured");
    assert!(panel.auto_translate_pending);
    assert!(!panel.translating);
    let _ = panel.update(service_loaded());
    assert!(panel.translating);
    assert!(!panel.auto_translate_pending);
    let revision = panel.revision;
    let _ = panel.update(service_loaded());
    assert_eq!(panel.revision, revision);

    let mut panel = TranslationPanel::new(Language::English);
    let _ = panel.update(Message::InputAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('x'),
    )));
    let _ = panel.update(service_loaded());
    assert!(!panel.translating);
}

#[test]
fn editing_clearing_or_changing_direction_cancels_automatic_submission_during_loading() {
    for message in [
        Message::InputAction(text_editor::Action::Edit(text_editor::Edit::Insert('x'))),
        Message::Clear,
        Message::SwapLanguages,
    ] {
        let mut panel = TranslationPanel::new(Language::English);
        let _ = panel.import_and_translate("captured");
        let _ = panel.update(message);
        let _ = panel.update(service_loaded());
        assert!(!panel.auto_translate_pending);
        assert!(!panel.translating);
    }
}

#[test]
fn configuration_failure_keeps_captured_text_and_does_not_submit() {
    let mut panel = TranslationPanel::new(Language::English);
    let _ = panel.import_and_translate("captured");
    let _ = panel.update(Message::ServiceLoaded(Err(translation::Error::Network)));
    assert_eq!(panel.input.text(), "captured");
    assert!(!panel.auto_translate_pending);
    assert!(!panel.translating);
    let _ = panel.import_and_translate("another capture");
    assert_eq!(panel.input.text(), "another capture");
    assert!(!panel.translating);
    assert!(matches!(panel.service, ServiceState::Failed(_)));
}

#[test]
fn user_actions_cancel_capture_while_navigation_and_service_loading_do_not() {
    for message in [
        Message::InputAction(text_editor::Action::Edit(text_editor::Edit::Insert('x'))),
        Message::Clear,
        Message::SwapLanguages,
        Message::Translate,
    ] {
        let mut app = App {
            panel: ready_panel(),
            ..App::default()
        };
        app.panel.import_text("original");
        let revision = start_capture(&mut app);
        let _ = app.update(AppMessage::Panel(message));
        let edited = app.panel.input.text();
        let _ = app.update(AppMessage::TextCaptured {
            revision,
            snapshot: crate::text_capture::Snapshot {
                primary: Ok("too late".into()),
                clipboard: Err(crate::text_capture::Error::NoText),
            },
        });
        assert_eq!(app.panel.input.text(), edited);
        assert!(app.capture.is_none());
    }

    let mut app = App::default();
    let revision = start_capture(&mut app);
    let _ = app.update(AppMessage::Panel(Message::InputAction(
        text_editor::Action::Move(text_editor::Motion::Left),
    )));
    let _ = app.update(AppMessage::Panel(Message::ServiceLoaded(Err(
        translation::Error::Network,
    ))));
    let _ = app.update(AppMessage::TextCaptured {
        revision,
        snapshot: crate::text_capture::Snapshot {
            primary: Ok("captured".into()),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert_eq!(app.panel.input.text(), "captured");
    assert!(matches!(app.panel.service, ServiceState::Failed(_)));
}

#[test]
fn unsuccessful_capture_preserves_source_and_translation() {
    let mut app = App {
        panel: ready_panel(),
        ..App::default()
    };
    app.panel.input = text_editor::Content::with_text("Hello");
    app.panel.output = text_editor::Content::with_text("你好");
    let revision = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision,
        snapshot: crate::text_capture::Snapshot {
            primary: Err(crate::text_capture::Error::NoText),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert_eq!(app.panel.input.text(), "Hello");
    assert_eq!(app.panel.output.text(), "你好");
    assert!(app.capture_notice.is_some());
    assert!(!app.panel.translating);
}

#[test]
#[cfg(not(target_os = "macos"))]
fn new_clipboard_replaces_an_old_selection_and_repeated_activation_keeps_the_new_result() {
    let mut app = App {
        panel: ready_panel(),
        ..App::default()
    };
    let sources = |clipboard: &str| crate::text_capture::Snapshot {
        primary: Ok("old selection".into()),
        clipboard: Ok(clipboard.into()),
    };
    let first = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision: first,
        snapshot: sources("old clipboard"),
    });
    let old_translation = app.panel.revision;
    let _ = app.update(AppMessage::Panel(Message::Translated {
        revision: old_translation,
        result: Ok("旧译文".into()),
    }));
    let second = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision: second,
        snapshot: sources("new CLI text"),
    });
    assert_eq!(app.panel.input.text(), "new CLI text");
    assert!(app.panel.translating);
    assert!(app.panel.output.is_empty());
    let current_translation = app.panel.revision;
    let _ = app.update(AppMessage::Panel(Message::Translated {
        revision: current_translation,
        result: Ok("新译文".into()),
    }));
    let third = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision: third,
        snapshot: sources("new CLI text"),
    });
    assert_eq!(app.panel.input.text(), "new CLI text");
    assert_eq!(app.panel.output.text(), "新译文");
    assert_eq!(app.panel.revision, current_translation);
    assert!(!app.panel.translating);
}

#[test]
#[cfg(target_os = "macos")]
fn mac_permission_failure_falls_back_without_hiding_the_permission_notice() {
    use crate::text_capture::{Error, Snapshot};
    let mut app = App {
        panel: ready_panel(),
        ..App::default()
    };
    let revision = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision,
        snapshot: Snapshot {
            primary: Err(Error::AccessibilityRequired),
            clipboard: Ok("copied text".into()),
        },
    });
    assert_eq!(app.panel.input.text(), "copied text");
    assert!(app.panel.translating);
    assert_eq!(app.capture_notice, Some(Error::AccessibilityRequired));
    let _ = app.update(AppMessage::SelectionPermissionChecked(Ok(true)));
    assert!(app.capture_notice.is_none());
    assert_eq!(app.panel.input.text(), "copied text");

    let revision = start_capture(&mut app);
    let _ = app.update(AppMessage::TextCaptured {
        revision,
        snapshot: Snapshot {
            primary: Ok("selected text".into()),
            clipboard: Ok("new copied text".into()),
        },
    });
    assert_eq!(app.panel.input.text(), "selected text");
    assert!(app.capture_notice.is_none());
}

#[test]
#[cfg(target_os = "macos")]
fn mac_late_permission_failure_cannot_replace_edited_text_or_add_a_notice() {
    use crate::text_capture::{Error, Snapshot};
    let mut app = App {
        panel: ready_panel(),
        ..App::default()
    };
    let revision = start_capture(&mut app);
    let _ = app.update(AppMessage::Panel(Message::Clear));
    let _ = app.update(AppMessage::TextCaptured {
        revision,
        snapshot: Snapshot {
            primary: Err(Error::AccessibilityRequired),
            clipboard: Ok("stale clipboard".into()),
        },
    });
    assert!(app.panel.input.is_empty());
    assert!(app.capture_notice.is_none());
}

#[test]
fn translation_requires_a_ready_service_nonempty_input_and_no_pending_request() {
    let mut panel = TranslationPanel {
        input: text_editor::Content::with_text("Hello"),
        ..TranslationPanel::new(Language::English)
    };
    let _ = panel.update(Message::Translate);
    assert!(!panel.translating);

    panel = ready_panel();
    for input in ["", " \n "] {
        panel.input = text_editor::Content::with_text(input);
        assert!(!panel.can_translate());
    }
    panel.input = text_editor::Content::with_text("Hello");
    assert!(panel.can_translate());
    let _ = panel.update(Message::Translate);
    assert!(panel.translating);
    assert!(!panel.can_translate());
    let revision = panel.revision;
    let _ = panel.update(Message::Translate);
    assert_eq!(panel.revision, revision);
}

#[test]
fn configuration_failure_survives_input_edits_and_keeps_translation_disabled() {
    let mut panel = TranslationPanel::new(Language::English);
    let _ = panel.update(Message::ServiceLoaded(Err(translation::Error::Config(
        crate::config::Error::Missing("config.toml".into()),
    ))));
    let _ = panel.update(Message::InputAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('x'),
    )));
    assert!(matches!(panel.service, ServiceState::Failed(_)));
    assert!(!panel.can_translate());
}

#[test]
fn request_failure_allows_retry_and_old_results_cannot_replace_new_request() {
    let mut panel = ready_panel();
    panel.input = text_editor::Content::with_text("Hello");
    let _ = panel.update(Message::Translate);
    let first_revision = panel.revision;
    let _ = panel.update(Message::Translated {
        revision: first_revision,
        result: Err(translation::Error::Http(429)),
    });
    assert!(!panel.translating);
    assert!(panel.can_translate());
    assert!(matches!(panel.error, Some(translation::Error::Http(429))));

    let _ = panel.update(Message::Translate);
    assert!(panel.error.is_none());
    for result in [Ok("old".into()), Err(translation::Error::Network)] {
        let _ = panel.update(Message::Translated {
            revision: first_revision,
            result,
        });
        assert!(panel.translating);
        assert!(panel.error.is_none());
        assert!(panel.output.is_empty());
    }
    let _ = panel.update(Message::Translated {
        revision: panel.revision,
        result: Ok("你好".into()),
    });
    assert_eq!(panel.output.text(), "你好");
    assert!(!panel.translating);
}

#[test]
fn editing_input_discards_both_stale_success_and_stale_error() {
    for result in [Ok("old".into()), Err(translation::Error::Timeout)] {
        let mut panel = ready_panel();
        panel.input = text_editor::Content::with_text("Hello");
        let _ = panel.update(Message::Translate);
        let revision = panel.revision;
        let _ = panel.update(Message::InputAction(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let _ = panel.update(Message::Translated { revision, result });
        assert!(!panel.translating);
        assert!(panel.output.is_empty());
        assert!(panel.error.is_none());
        assert!(panel.can_translate());
    }
}

#[test]
fn changing_interface_language_preserves_text_and_translation_direction() {
    let mut app = App {
        language: Language::English,
        panel: TranslationPanel {
            input: text_editor::Content::with_text("Hello\nWorld"),
            output: text_editor::Content::with_text("你好\n世界"),
            ..TranslationPanel::new(Language::English)
        },
        ..App::default()
    };

    let _ = app.update(AppMessage::LanguageChanged(Language::Chinese));

    assert_eq!(app.title(), "Yeravich · 翻译");
    assert_eq!(app.panel.input.text(), "Hello\nWorld");
    assert_eq!(app.panel.output.text(), "你好\n世界");
    assert_eq!(app.panel.source_language, Language::English);
}

#[test]
fn output_is_read_only_and_accepts_current_translation_results() {
    let mut app = App {
        language: Language::English,
        panel: TranslationPanel {
            translating: true,
            ..TranslationPanel::new(Language::English)
        },
        ..App::default()
    };
    let _ = app.update(AppMessage::Panel(Message::Translated {
        revision: app.panel.revision,
        result: Ok("你好，世界".into()),
    }));
    let _ = app.update(AppMessage::Panel(Message::OutputAction(
        text_editor::Action::Edit(text_editor::Edit::Insert('x')),
    )));

    assert_eq!(app.panel.output.text(), "你好，世界");
    assert!(!app.panel.translating);
}

#[test]
fn clearing_or_changing_direction_discards_in_flight_results() {
    for action in [Message::Clear, Message::SwapLanguages] {
        let mut panel = TranslationPanel {
            input: text_editor::Content::with_text("Hello"),
            translating: true,
            ..TranslationPanel::new(Language::English)
        };
        let revision = panel.revision;
        let _ = panel.update(action);
        let _ = panel.update(Message::Translated {
            revision,
            result: Ok("过期的译文".into()),
        });

        assert!(panel.output.is_empty());
        assert!(!panel.translating);
    }
}

#[test]
fn window_lifecycle_preserves_text_and_translation_direction() {
    use crate::app::{ShortcutStatus, window::WindowState};

    for wayland in [false, true] {
        let id = iced::window::Id::unique();
        let mut app = App {
            panel: TranslationPanel {
                input: text_editor::Content::with_text("Hello\nWorld"),
                output: text_editor::Content::with_text("你好\n世界"),
                ..TranslationPanel::new(Language::English)
            },
            window: WindowState::Visible(id),
            wayland,
            shortcut_status: ShortcutStatus::Ready("Ctrl+Alt+Space".into()),
            ..App::default()
        };

        let _ = app.update(AppMessage::CloseRequested(id));
        if wayland {
            let _ = app.update(AppMessage::WindowClosed(id));
        }
        let _ = app.update(AppMessage::Shortcut(crate::shortcut::Event::Activated));

        assert_eq!(app.panel.input.text(), "Hello\nWorld");
        assert_eq!(app.panel.output.text(), "你好\n世界");
        assert_eq!(app.panel.source_language, Language::English);
    }
}
