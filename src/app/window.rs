use iced::{Task, widget, window};

use super::{App, Message, ShortcutStatus, translation_panel};
use crate::theme::tokens::layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowState {
    Absent,
    Opening(window::Id),
    Visible(window::Id),
    Hidden(window::Id),
    Closing { id: window::Id, reopen: bool },
}

impl WindowState {
    pub fn id(self) -> Option<window::Id> {
        match self {
            Self::Absent => None,
            Self::Opening(id) | Self::Visible(id) | Self::Hidden(id) | Self::Closing { id, .. } => {
                Some(id)
            }
        }
    }
}

impl App {
    pub(super) fn can_hide(&self) -> bool {
        self.tray_ready || matches!(self.shortcut_status, ShortcutStatus::Ready(_))
    }

    pub(super) fn show_window(&mut self) -> Task<Message> {
        match self.window {
            WindowState::Absent => {
                let (id, open) = window::open(window::Settings {
                    size: self.window_size,
                    min_size: Some(layout::MIN_WINDOW_SIZE),
                    position: window::Position::Centered,
                    exit_on_close_request: false,
                    #[cfg(target_os = "linux")]
                    platform_specific: window::settings::PlatformSpecific {
                        application_id: "com.zonowry.yeravich".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
                // 在异步创建完成前就记录 ID，连续热键不会重复创建窗口。
                self.window = WindowState::Opening(id);
                self.focused = false;
                open.map(Message::WindowOpened)
            }
            WindowState::Hidden(id) => {
                self.window = WindowState::Visible(id);
                window::set_mode(id, window::Mode::Windowed)
                    .chain(window::minimize(id, false))
                    .chain(Self::focus_input(id))
            }
            WindowState::Visible(id) if self.wayland && !self.focused => {
                // winit 也不支持在 Wayland 主动聚焦已有窗口，重新映射交给桌面激活。
                self.window = WindowState::Closing { id, reopen: true };
                window::close(id)
            }
            WindowState::Visible(id) => window::minimize(id, false).chain(Self::focus_input(id)),
            WindowState::Opening(_) => Task::none(),
            WindowState::Closing { id, .. } => {
                self.window = WindowState::Closing { id, reopen: true };
                Task::none()
            }
        }
    }

    pub(super) fn hide_window(&mut self, id: window::Id) -> Task<Message> {
        if self.window != WindowState::Visible(id) {
            return Task::none();
        }
        if !self.can_hide() {
            self.close_blocked = true;
            return Task::none();
        }

        self.cancel_capture("window hidden");
        self.focused = false;
        if self.wayland {
            // winit 的 set_visible 不支持 Wayland。销毁窗口但保留 App；
            // 等收到 Closed 后再允许重建，避免打开与关闭任务交错。
            self.window = WindowState::Closing { id, reopen: false };
            window::close(id)
        } else {
            self.window = WindowState::Hidden(id);
            window::set_mode(id, window::Mode::Hidden)
        }
    }

    pub(super) fn window_closed(&mut self, id: window::Id) -> Task<Message> {
        if self.window.id() != Some(id) {
            return Task::none();
        }
        let reopen =
            matches!(self.window, WindowState::Closing { reopen: true, .. }) || !self.can_hide();
        if !reopen {
            self.cancel_capture("window closed");
        }
        self.window = WindowState::Absent;
        if reopen {
            self.show_window()
        } else {
            Task::none()
        }
    }

    pub(super) fn focus_input(id: window::Id) -> Task<Message> {
        window::gain_focus(id).chain(widget::operation::focus(translation_panel::INPUT_ID))
    }
}
