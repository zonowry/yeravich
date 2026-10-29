use iced::Task;

use super::{App, Message, window::WindowState};
use crate::text_capture;

pub(super) struct Capture {
    pub revision: u64,
    handle: Option<iced::task::Handle>,
    #[cfg(target_os = "macos")]
    source_pid: Option<i32>,
}

impl App {
    pub(super) fn show_with_text(&mut self) -> Task<Message> {
        eprintln!(
            "[capture] shortcut activated window={:?} focused={}",
            self.window, self.focused
        );
        // 已在应用中编辑时，热键只恢复焦点；不读取自己的选区覆盖原文。
        if !(matches!(self.window, WindowState::Visible(_)) && self.focused) {
            self.cancel_capture("new activation");
            self.capture_revision += 1;
            self.capture = Some(Capture {
                revision: self.capture_revision,
                handle: None,
                #[cfg(target_os = "macos")]
                source_pid: text_capture::macos::frontmost_application(),
            });
            // macOS 的来源应用可能在失焦时清空选区。后台读完再显示/聚焦，
            // 只固定 PID 并不能保留来源控件的选区；读取仍不阻塞事件线程。
            #[cfg(target_os = "macos")]
            return self.read_pending_capture();
        } else {
            eprintln!("[capture] skipped: application already focused");
        }
        // 其他平台可并行显示与读取；编辑等操作可随时使读取结果失效。
        Task::batch([self.show_window(), self.read_pending_capture()])
    }

    pub(super) fn read_pending_capture(&mut self) -> Task<Message> {
        // Xwayland 的原生剪贴板访问可能需要先取得焦点。等待实际 Focused
        // 事件，而不是假设 gain_focus 任务完成就代表桌面已授予焦点。
        if !self.focused && !text_capture::reads_without_focus(self.wayland) {
            if let Some(capture) = &self.capture {
                eprintln!("[capture #{}] waiting for window focus", capture.revision);
            }
            return Task::none();
        }
        let Some(capture) = &mut self.capture else {
            return Task::none();
        };
        if capture.handle.is_some() {
            return Task::none();
        }
        let revision = capture.revision;
        eprintln!(
            "[capture #{revision}] reading primary and clipboard; independent_reader={}",
            text_capture::reads_without_focus(self.wayland)
        );
        #[cfg(target_os = "macos")]
        let task = text_capture::macos::read(capture.source_pid);
        #[cfg(not(target_os = "macos"))]
        let task = text_capture::read(self.wayland);
        let (task, handle) = task
            .map(move |snapshot| Message::TextCaptured { revision, snapshot })
            .abortable();
        capture.handle = Some(handle.abort_on_drop());
        task
    }

    pub(super) fn cancel_capture(&mut self, reason: &str) {
        if let Some(capture) = &self.capture {
            eprintln!("[capture #{}] cancelled: {reason}", capture.revision);
        }
        self.capture = None;
        self.capture_notice = None;
    }

    pub(super) fn text_captured(
        &mut self,
        revision: u64,
        snapshot: text_capture::Snapshot,
    ) -> Task<Message> {
        if self.capture.as_ref().map(|pending| pending.revision) != Some(revision) {
            eprintln!("[capture #{revision}] ignored stale sources");
            return Task::none();
        }
        let source = snapshot.select(self.capture_snapshot.as_ref(), self.capture_source);
        eprintln!(
            "[capture #{revision}] {snapshot:?}, selected={source:?}, primary_changed={}, clipboard_changed={}",
            self.capture_snapshot
                .as_ref()
                .is_none_or(|old| old.primary != snapshot.primary),
            self.capture_snapshot
                .as_ref()
                .is_none_or(|old| old.clipboard != snapshot.clipboard)
        );
        self.capture = None;
        #[cfg(target_os = "macos")]
        {
            // 剪贴板兜底成功也要解释为什么没有使用选区。
            self.capture_notice = match snapshot.primary {
                Err(text_capture::Error::AccessibilityRequired) => {
                    Some(text_capture::Error::AccessibilityRequired)
                }
                Err(text_capture::Error::SelectionUnavailable) => {
                    Some(text_capture::Error::SelectionUnavailable)
                }
                Err(text_capture::Error::NoText) if snapshot.clipboard.is_ok() => {
                    Some(text_capture::Error::ClipboardFallback)
                }
                _ => None,
            };
        }
        let task = match snapshot.text(source) {
            Ok(text) => {
                self.capture_source = Some(source);
                self.panel.import_and_translate(text).map(Message::Panel)
            }
            Err(error) => {
                eprintln!("[capture #{revision}] failed: {error:?}");
                self.capture_notice = self.capture_notice.or(Some(error));
                Task::none()
            }
        };
        self.capture_snapshot = Some(snapshot);
        // 只让仍有效的读取触发唤出。取消或旧结果不能再次把窗口抢到前台。
        #[cfg(target_os = "macos")]
        let task = Task::batch([self.show_window(), task]);
        task
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::translation_panel;

    #[test]
    #[cfg(target_os = "macos")]
    fn mac_waits_for_the_current_capture_before_restoring_the_window() {
        let id = iced::window::Id::unique();
        let mut app = App {
            window: WindowState::Hidden(id),
            ..App::default()
        };
        let _ = app.show_with_text();
        let first = app.capture.as_ref().unwrap().revision;
        assert_eq!(app.window, WindowState::Hidden(id));
        assert!(app.capture.as_ref().unwrap().handle.is_some());
        let _ = app.show_with_text();
        let current = app.capture.as_ref().unwrap().revision;
        let snapshot = || text_capture::Snapshot {
            primary: Ok("selected".into()),
            clipboard: Ok("clipboard".into()),
        };
        assert_eq!(app.text_captured(first, snapshot()).units(), 0);
        assert_eq!(app.window, WindowState::Hidden(id));
        assert!(app.capture_snapshot.is_none());
        let _ = app.text_captured(current, snapshot());
        assert_eq!(app.window, WindowState::Visible(id));
        assert!(app.capture.is_none());
        assert_eq!(app.capture_source, Some(text_capture::Source::Primary));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn mac_failures_still_restore_the_window_but_cancelled_reads_do_not() {
        use text_capture::Error;
        for error in [
            Error::NoText,
            Error::AccessibilityRequired,
            Error::SelectionUnavailable,
        ] {
            let id = iced::window::Id::unique();
            let mut app = App {
                window: WindowState::Hidden(id),
                ..App::default()
            };
            let _ = app.show_with_text();
            let revision = app.capture.as_ref().unwrap().revision;
            let snapshot = || text_capture::Snapshot {
                primary: Err(error),
                clipboard: Err(Error::NoText),
            };
            let _ = app.text_captured(revision, snapshot());
            assert_eq!(app.window, WindowState::Visible(id));
            assert_eq!(app.capture_notice, Some(error));

            app.window = WindowState::Hidden(id);
            let _ = app.show_with_text();
            let revision = app.capture.as_ref().unwrap().revision;
            app.cancel_capture("test cancellation");
            assert_eq!(app.text_captured(revision, snapshot()).units(), 0);
            assert_eq!(app.window, WindowState::Hidden(id));
            assert!(app.capture_notice.is_none());
        }
    }

    #[test]
    fn focus_starts_a_waiting_capture_once_and_editing_cancels_it() {
        let id = iced::window::Id::unique();
        let mut app = App {
            window: WindowState::Visible(id),
            capture: Some(Capture {
                revision: 1,
                handle: None,
                #[cfg(target_os = "macos")]
                source_pid: None,
            }),
            ..App::default()
        };
        let _ = app.update(Message::WindowFocused(id, true));
        assert!(app.capture.as_ref().unwrap().handle.is_some());
        let _ = app.update(Message::WindowFocused(id, true));
        assert_eq!(app.capture.as_ref().unwrap().revision, 1);

        app.capture = Some(Capture {
            revision: 2,
            handle: None,
            #[cfg(target_os = "macos")]
            source_pid: None,
        });
        let _ = app.update(Message::Panel(translation_panel::Message::Clear));
        let _ = app.update(Message::WindowFocused(id, true));
        assert!(app.capture.is_none());
    }
}
