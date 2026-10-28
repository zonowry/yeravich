use super::*;

fn ready_app(wayland: bool) -> (App, iced_window::Id) {
    let id = iced_window::Id::unique();
    (
        App {
            window: WindowState::Visible(id),
            wayland,
            shortcut_status: ShortcutStatus::Ready("Ctrl+Alt+Space".into()),
            ..App::default()
        },
        id,
    )
}

#[test]
fn configuration_before_window_readiness_waits_to_register() {
    let id = iced_window::Id::unique();
    let mut app = App {
        window: WindowState::Opening(id),
        ..App::default()
    };
    let binding = shortcut::Binding::parse("Ctrl+Shift+T").unwrap();
    let _ = app.update(Message::ConfigurationLoaded {
        service: Err(translation::Error::Client),
        show_window: Ok(binding),
    });
    assert_eq!(app.pending_shortcut, Some(binding));
    assert!(app.registration.is_none());
    assert!(matches!(app.shortcut_status, ShortcutStatus::Pending));
    // 已有注册时，后续窗口就绪消息也不能重新注册或消费配置。
    app.registration = Some(shortcut::Registration::default());
    let _ = app.update(Message::WindowReady(id, false));
    assert_eq!(app.pending_shortcut, Some(binding));
}

#[test]
fn window_before_configuration_stays_visible_and_bad_config_keeps_the_guard() {
    let id = iced_window::Id::unique();
    let mut app = App {
        window: WindowState::Opening(id),
        ..App::default()
    };
    let _ = app.update(Message::WindowReady(id, false));
    assert!(app.registration.is_none());
    let _ = app.update(Message::CloseRequested(id));
    assert_eq!(app.window, WindowState::Visible(id));
    let error = config::Error::Invalid {
        path: "config.toml".into(),
        field: "shortcuts.show_window",
    };
    let _ = app.update(Message::ConfigurationLoaded {
        service: Err(translation::Error::Config(error.clone())),
        show_window: Err(error),
    });
    assert!(matches!(app.shortcut_status, ShortcutStatus::Failed));
    assert!(app.pending_shortcut.is_none());
    let _ = app.update(Message::CloseRequested(id));
    assert_eq!(app.window, WindowState::Visible(id));
}

#[test]
fn late_configuration_cannot_register_a_shortcut_after_exit() {
    let mut app = App {
        exiting: true,
        ..App::default()
    };
    let _ = app.update(Message::ConfigurationLoaded {
        service: Err(translation::Error::Client),
        show_window: Ok(shortcut::Binding::default()),
    });
    assert!(app.pending_shortcut.is_none());
    assert!(app.registration.is_none());
}

fn key_press(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        modified_key: key.clone(),
        key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    })
}

#[test]
fn escape_closes_the_event_window_without_exiting() {
    for wayland in [false, true] {
        let (mut app, id) = ready_app(wayland);
        app.registration = Some(shortcut::Registration::default());
        let message = App::keyboard_shortcut(
            key_press(
                keyboard::key::Named::Escape.into(),
                keyboard::Modifiers::empty(),
            ),
            event::Status::Ignored,
            id,
        )
        .expect("Escape requests closing the window");
        let _ = app.update(message);
        assert_eq!(
            app.window,
            if wayland {
                WindowState::Closing { id, reopen: false }
            } else {
                WindowState::Hidden(id)
            }
        );
        assert!(!app.exiting);
        assert!(app.registration.is_some());
    }
}

#[test]
fn escape_preserves_the_close_guard_when_shortcuts_are_unavailable() {
    for status in [ShortcutStatus::Pending, ShortcutStatus::Failed] {
        let (mut app, id) = ready_app(true);
        app.shortcut_status = status;
        let message = App::keyboard_shortcut(
            key_press(
                keyboard::key::Named::Escape.into(),
                keyboard::Modifiers::empty(),
            ),
            event::Status::Ignored,
            id,
        )
        .expect("Escape requests closing the window");
        let _ = app.update(message);
        assert_eq!(app.window, WindowState::Visible(id));
        assert!(app.close_blocked);
        assert!(!app.exiting);
    }
}

#[test]
fn captured_or_modified_escape_does_not_close_and_command_q_still_exits() {
    let id = iced_window::Id::unique();
    for (status, modifiers) in [
        (event::Status::Captured, keyboard::Modifiers::empty()),
        (event::Status::Ignored, keyboard::Modifiers::SHIFT),
        (event::Status::Ignored, keyboard::Modifiers::CTRL),
        (event::Status::Ignored, keyboard::Modifiers::ALT),
        (event::Status::Ignored, keyboard::Modifiers::LOGO),
    ] {
        assert!(
            App::keyboard_shortcut(
                key_press(keyboard::key::Named::Escape.into(), modifiers),
                status,
                id,
            )
            .is_none()
        );
    }
    for status in [event::Status::Ignored, event::Status::Captured] {
        assert!(matches!(
            App::keyboard_shortcut(
                key_press(
                    keyboard::Key::Character("q".into()),
                    keyboard::Modifiers::COMMAND
                ),
                status,
                id,
            ),
            Some(Message::Exit)
        ));
    }
    assert!(
        App::keyboard_shortcut(
            key_press(
                keyboard::key::Named::Enter.into(),
                keyboard::Modifiers::empty()
            ),
            event::Status::Ignored,
            id,
        )
        .is_none()
    );
}

#[test]
fn closing_and_reopening_reuses_a_supported_window() {
    let (mut app, id) = ready_app(false);
    let _ = app.update(Message::CloseRequested(id));
    assert_eq!(app.window, WindowState::Hidden(id));

    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    assert_eq!(app.window, WindowState::Visible(id));
}

#[test]
fn wayland_reopens_only_after_the_previous_window_has_closed() {
    let (mut app, id) = ready_app(true);
    let _ = app.update(Message::CloseRequested(id));
    assert_eq!(app.window, WindowState::Closing { id, reopen: false });

    for _ in 0..3 {
        let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
        assert_eq!(app.window, WindowState::Closing { id, reopen: true });
    }
    let _ = app.update(Message::WindowClosed(id));
    let WindowState::Opening(new_id) = app.window else {
        panic!("the pending shortcut should reopen the window");
    };
    assert_ne!(new_id, id);

    // 创建尚未完成时的热键，以及旧窗口迟到的事件，都不能创建第二个窗口。
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    let _ = app.update(Message::WindowClosed(id));
    assert_eq!(app.window, WindowState::Opening(new_id));
}

#[test]
fn closing_without_a_working_shortcut_keeps_the_window_accessible() {
    for status in [ShortcutStatus::Pending, ShortcutStatus::Failed] {
        let (mut app, id) = ready_app(false);
        app.shortcut_status = status;
        let _ = app.update(Message::CloseRequested(id));
        assert_eq!(app.window, WindowState::Visible(id));
        assert!(app.close_blocked);
    }
}

#[test]
fn successful_registration_without_a_description_allows_closing_before_activation() {
    for wayland in [false, true] {
        let (mut app, id) = ready_app(wayland);
        app.shortcut_status = ShortcutStatus::Pending;
        let _ = app.update(Message::Shortcut(shortcut::Event::Ready(String::new())));
        let _ = app.update(Message::CloseRequested(id));
        assert!(!app.close_blocked);
        assert_eq!(
            app.window,
            if wayland {
                WindowState::Closing { id, reopen: false }
            } else {
                WindowState::Hidden(id)
            }
        );
    }
}

#[test]
fn wayland_reuses_a_focused_window_and_remaps_an_unfocused_one() {
    let (mut app, id) = ready_app(true);
    let _ = app.update(Message::WindowFocused(id, true));
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    assert_eq!(app.window, WindowState::Visible(id));

    let _ = app.update(Message::WindowFocused(id, false));
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    assert_eq!(app.window, WindowState::Closing { id, reopen: true });
}

#[test]
fn losing_the_shortcut_restores_a_hidden_window() {
    let (mut app, id) = ready_app(false);
    let _ = app.update(Message::CloseRequested(id));
    let _ = app.update(Message::Shortcut(shortcut::Event::Failed(
        "disconnected".into(),
    )));
    assert_eq!(app.window, WindowState::Visible(id));
    assert!(matches!(app.shortcut_status, ShortcutStatus::Failed));
}

#[test]
fn losing_the_shortcut_while_closing_wayland_schedules_reopening() {
    let (mut app, id) = ready_app(true);
    let _ = app.update(Message::CloseRequested(id));
    let _ = app.update(Message::Shortcut(shortcut::Event::Failed(
        "disconnected".into(),
    )));
    let _ = app.update(Message::WindowClosed(id));
    assert!(matches!(app.window, WindowState::Opening(_)));
}

#[test]
fn wayland_keeps_window_size_and_waits_for_a_shortcut_after_closing() {
    let (mut app, id) = ready_app(true);
    let size = Size::new(900.0, 800.0);
    let _ = app.update(Message::WindowResized(id, size));
    let _ = app.update(Message::CloseRequested(id));
    let _ = app.update(Message::WindowClosed(id));
    assert_eq!(app.window, WindowState::Absent);
    assert_eq!(app.window_size, size);

    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    assert!(matches!(app.window, WindowState::Opening(_)));
    assert_eq!(app.window_size, size);
}

#[test]
fn an_empty_description_update_does_not_reopen_or_block_the_window() {
    for wayland in [false, true] {
        let (mut app, id) = ready_app(wayland);
        let _ = app.update(Message::CloseRequested(id));
        if wayland {
            let _ = app.update(Message::WindowClosed(id));
        }
        let hidden = app.window;
        let _ = app.update(Message::Shortcut(shortcut::Event::Ready(String::new())));
        assert_eq!(app.window, hidden);
        assert!(matches!(app.shortcut_status, ShortcutStatus::Ready(_)));

        let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
        let visible_id = app.window.id().expect("activation restores a window");
        if wayland {
            app.registration = Some(shortcut::Registration::default());
            let _ = app.update(Message::WindowReady(visible_id, true));
        }
        let _ = app.update(Message::CloseRequested(visible_id));
        assert!(!app.close_blocked);
        assert!(matches!(
            app.window,
            WindowState::Hidden(_) | WindowState::Closing { reopen: false, .. }
        ));
    }
}

#[test]
fn exit_releases_registration_and_ignores_further_activations() {
    let (mut app, id) = ready_app(false);
    app.registration = Some(shortcut::Registration::default());
    let _ = app.update(Message::CloseRequested(id));
    let _ = app.update(Message::Exit);
    assert!(app.exiting);
    assert!(app.registration.is_none());

    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    assert_eq!(app.window, WindowState::Hidden(id));
}

#[test]
fn tray_allows_closing_without_a_shortcut_and_restores_the_same_state() {
    for wayland in [false, true] {
        let (mut app, id) = ready_app(wayland);
        app.shortcut_status = ShortcutStatus::Failed;
        let _ = app.update(Message::CloseRequested(id));
        assert!(app.close_blocked);
        let _ = app.update(Message::Tray(tray::Event::Ready));
        assert!(!app.close_blocked);
        let _ = app.update(Message::CloseRequested(id));
        if wayland {
            let _ = app.update(Message::WindowClosed(id));
            assert_eq!(app.window, WindowState::Absent);
        } else {
            assert_eq!(app.window, WindowState::Hidden(id));
        }
        let revision = app.capture_revision;
        let _ = app.update(Message::Tray(tray::Event::Show));
        assert!(matches!(
            app.window,
            WindowState::Visible(_) | WindowState::Opening(_)
        ));
        assert_eq!(app.capture_revision, revision);
        assert!(app.capture.is_none());
        assert!(!app.exiting);
    }
}

#[test]
fn tray_reopening_without_a_shortcut_does_not_repeat_the_startup_capture() {
    let first_id = iced_window::Id::unique();
    let mut app = App {
        window: WindowState::Opening(first_id),
        shortcut_status: ShortcutStatus::Failed,
        tray_ready: true,
        ..App::default()
    };
    let first = app.update(Message::WindowReady(first_id, true));
    assert!(app.capture_baseline_started);
    assert!(app.registration.is_none());
    let _ = app.update(Message::CloseRequested(first_id));
    let _ = app.update(Message::WindowClosed(first_id));
    let _ = app.update(Message::Tray(tray::Event::Show));
    let second_id = app.window.id().unwrap();
    let second = app.update(Message::WindowReady(second_id, true));
    assert!(
        first.units() > second.units(),
        "startup read must only run once, even before its result arrives"
    );
}

#[test]
fn tray_show_cancels_pending_capture_and_ignores_late_text() {
    let (mut app, _) = ready_app(true);
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    let revision = app.capture.as_ref().unwrap().revision;
    let _ = app.update(Message::Tray(tray::Event::Show));
    assert!(app.capture.is_none());
    let _ = app.update(Message::TextCaptured {
        revision,
        snapshot: text_capture::Snapshot {
            primary: Ok("late selection".into()),
            clipboard: Err(text_capture::Error::NoText),
        },
    });
    assert!(app.capture_snapshot.is_none());
}

#[test]
fn losing_both_restore_paths_reopens_the_window_in_either_order() {
    for wayland in [false, true] {
        for tray_first in [false, true] {
            let (mut app, id) = ready_app(wayland);
            let _ = app.update(Message::Tray(tray::Event::Ready));
            let _ = app.update(Message::CloseRequested(id));
            if wayland {
                let _ = app.update(Message::WindowClosed(id));
            }
            let hidden = app.window;
            let tray_failed = Message::Tray(tray::Event::Unavailable("offline".into()));
            let shortcut_failed = Message::Shortcut(shortcut::Event::Failed("offline".into()));
            let (first, second) = if tray_first {
                (tray_failed, shortcut_failed)
            } else {
                (shortcut_failed, tray_failed)
            };
            let _ = app.update(first);
            assert_eq!(app.window, hidden);
            let _ = app.update(second);
            assert!(matches!(
                app.window,
                WindowState::Visible(_) | WindowState::Opening(_)
            ));
            assert!(!app.can_hide());
        }
    }
}

#[test]
fn losing_the_tray_while_wayland_is_closing_schedules_reopening() {
    let (mut app, id) = ready_app(true);
    app.shortcut_status = ShortcutStatus::Failed;
    let _ = app.update(Message::Tray(tray::Event::Ready));
    let _ = app.update(Message::CloseRequested(id));
    let _ = app.update(Message::Tray(tray::Event::Unavailable("offline".into())));
    assert_eq!(app.window, WindowState::Closing { id, reopen: true });
    let _ = app.update(Message::WindowClosed(id));
    assert!(matches!(app.window, WindowState::Opening(_)));
}

#[test]
fn tray_exit_waits_for_both_resources_and_ignores_late_events() {
    for tray_first in [false, true] {
        let (mut app, id) = ready_app(false);
        app.registration = Some(shortcut::Registration::default());
        let _ = app.update(Message::CloseRequested(id));
        let _ = app.update(Message::Tray(tray::Event::Exit));
        assert!(app.exiting);
        assert!(app.registration.is_none());
        assert!(app.tray.is_none());
        for late in [
            Message::StartTray,
            Message::Tray(tray::Event::Ready),
            Message::Tray(tray::Event::Show),
        ] {
            let _ = app.update(late);
        }
        assert!(app.tray.is_none());
        assert!(!app.tray_ready);
        assert_eq!(app.window, WindowState::Hidden(id));

        let tray_stopped = Message::Tray(tray::Event::Stopped);
        let shortcut_stopped = Message::Shortcut(shortcut::Event::Stopped);
        let (first, second) = if tray_first {
            (tray_stopped, shortcut_stopped)
        } else {
            (shortcut_stopped, tray_stopped)
        };
        assert_eq!(app.update(first).units(), 0);
        assert!(!(app.tray_stopped && app.shortcut_stopped));
        let _ = app.update(second);
        assert!(app.tray_stopped && app.shortcut_stopped);
    }
}

#[test]
fn capture_runs_only_when_summoning_from_outside_and_ignores_old_errors() {
    let (mut app, id) = ready_app(true);
    let _ = app.update(Message::WindowFocused(id, true));
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    assert!(app.capture.is_none());

    let _ = app.update(Message::WindowFocused(id, false));
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    let first = app.capture.as_ref().unwrap().revision;
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    let second = app.capture.as_ref().unwrap().revision;
    assert_ne!(first, second);
    let _ = app.update(Message::TextCaptured {
        revision: first,
        snapshot: crate::text_capture::Snapshot {
            primary: Err(text_capture::Error::NoText),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert_eq!(app.capture.as_ref().unwrap().revision, second);
    assert!(app.capture_notice.is_none());
    let _ = app.update(Message::TextCaptured {
        revision: second,
        snapshot: crate::text_capture::Snapshot {
            primary: Err(text_capture::Error::NoText),
            clipboard: Err(crate::text_capture::Error::NoText),
        },
    });
    assert!(app.capture.is_none());
    assert_eq!(app.capture_notice, Some(text_capture::Error::NoText));
}

#[test]
fn closing_or_exiting_cancels_capture_and_ignores_queued_results() {
    for wayland in [true, false] {
        for exit in [false, true] {
            let (mut app, id) = ready_app(wayland);
            app.window = WindowState::Absent;
            let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
            let revision = app.capture.as_ref().unwrap().revision;
            app.window = WindowState::Visible(id);
            let _ = app.update(if exit {
                Message::Exit
            } else {
                Message::CloseRequested(id)
            });
            assert!(app.capture.is_none());
            let _ = app.update(Message::TextCaptured {
                revision,
                snapshot: crate::text_capture::Snapshot {
                    primary: Err(text_capture::Error::NoText),
                    clipboard: Err(crate::text_capture::Error::NoText),
                },
            });
            assert!(app.capture_notice.is_none());
        }
    }
}

#[test]
fn late_startup_snapshot_cannot_replace_an_activated_capture() {
    let mut app = App::default();
    let baseline = text_capture::Snapshot {
        primary: Ok("old".into()),
        clipboard: Ok("old clipboard".into()),
    };
    let _ = app.update(Message::CaptureBaseline(baseline));
    let _ = app.update(Message::Shortcut(shortcut::Event::Activated));
    let _ = app.update(Message::CaptureBaseline(text_capture::Snapshot {
        primary: Ok("late".into()),
        clipboard: Ok("late clipboard".into()),
    }));
    assert_eq!(
        app.capture_snapshot.as_ref().unwrap().primary,
        Ok("old".into())
    );
    let revision = app.capture.as_ref().unwrap().revision;
    let _ = app.update(Message::Panel(translation_panel::Message::Clear));
    let _ = app.update(Message::TextCaptured {
        revision,
        snapshot: text_capture::Snapshot {
            primary: Ok("cancelled".into()),
            clipboard: Ok("cancelled".into()),
        },
    });
    assert_eq!(
        app.capture_snapshot.as_ref().unwrap().primary,
        Ok("old".into())
    );
}
