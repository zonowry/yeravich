mod capture;
mod translation_panel;
mod view;
mod window;

#[cfg(test)]
mod tests;

use iced::{Event, Size, Subscription, Task, event, keyboard, window as iced_window};
use rust_i18n::t;

use crate::i18n::{Langs, Language};
use crate::{config, shortcut, text_capture, theme::tokens::layout, translation, tray};
use translation_panel::TranslationPanel;
use window::WindowState;

pub struct App {
    language: Language,
    panel: TranslationPanel,
    window: WindowState,
    window_size: Size,
    wayland: bool,
    focused: bool,
    registration: Option<shortcut::Registration>,
    pending_shortcut: Option<shortcut::Binding>,
    shortcut_status: ShortcutStatus,
    tray: Option<tray::Registration>,
    tray_ready: bool,
    tray_failed: bool,
    close_blocked: bool,
    exiting: bool,
    shortcut_stopped: bool,
    tray_stopped: bool,
    capture_revision: u64,
    capture_baseline_started: bool,
    capture: Option<capture::Capture>,
    capture_notice: Option<text_capture::Error>,
    capture_snapshot: Option<text_capture::Snapshot>,
    capture_source: Option<text_capture::Source>,
}

#[derive(Debug, Default)]
enum ShortcutStatus {
    #[default]
    Pending,
    Ready(String),
    Failed,
}

#[derive(Debug, Clone)]
pub enum Message {
    #[cfg(target_os = "macos")]
    RequestSelectionPermission,
    #[cfg(target_os = "macos")]
    SelectionPermissionChecked(Result<bool, text_capture::Error>),
    ConfigurationLoaded {
        service: Result<translation::Service, translation::Error>,
        show_window: Result<shortcut::Binding, config::Error>,
    },
    LanguageChanged(Language),
    Panel(translation_panel::Message),
    WindowOpened(iced_window::Id),
    WindowReady(iced_window::Id, bool),
    CloseRequested(iced_window::Id),
    WindowClosed(iced_window::Id),
    WindowResized(iced_window::Id, Size),
    WindowFocused(iced_window::Id, bool),
    Shortcut(shortcut::Event),
    StartTray,
    Tray(tray::Event),
    CaptureBaseline(text_capture::Snapshot),
    TextCaptured {
        revision: u64,
        snapshot: text_capture::Snapshot,
    },
    Exit,
}

impl Default for App {
    fn default() -> Self {
        let language = Language::default();

        Self {
            language,
            panel: TranslationPanel::new(language),
            window: WindowState::Absent,
            window_size: layout::WINDOW_SIZE,
            wayland: false,
            focused: false,
            registration: None,
            pending_shortcut: None,
            shortcut_status: ShortcutStatus::Pending,
            tray: None,
            tray_ready: false,
            tray_failed: false,
            close_blocked: false,
            exiting: false,
            shortcut_stopped: false,
            tray_stopped: false,
            capture_revision: 0,
            capture_baseline_started: false,
            capture: None,
            capture_notice: None,
            capture_snapshot: None,
            capture_source: None,
        }
    }
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        eprintln!(
            "[yeravich] started pid={} version={}",
            std::process::id(),
            env!("CARGO_PKG_VERSION")
        );
        let mut app = Self::default();
        let task = Task::batch([
            app.show_window().chain(Task::done(Message::StartTray)),
            Self::load_configuration(),
        ]);
        #[cfg(target_os = "macos")]
        let task = Task::batch([
            task,
            text_capture::macos::permission(false).map(Message::SelectionPermissionChecked),
        ]);
        (app, task)
    }

    fn load_configuration() -> Task<Message> {
        Task::perform(
            async {
                // 配置只读取一次，网络客户端也在后台创建；窗口打开与它们并行。
                match config::load().await {
                    Ok(config) => (
                        translation::Service::new(config.translation),
                        Ok(config.show_window),
                    ),
                    Err(error) => (Err(translation::Error::Config(error.clone())), Err(error)),
                }
            },
            |(service, show_window)| Message::ConfigurationLoaded {
                service,
                show_window,
            },
        )
    }

    fn register_shortcut(&mut self) -> Task<Message> {
        // 窗口就绪与配置读取的完成顺序不确定。等两者都就绪后，
        // 在窗口事件线程注册；take 保证不会在唤回窗口时重复注册。
        if self.registration.is_some() || !matches!(self.window, WindowState::Visible(_)) {
            return Task::none();
        }
        let Some(binding) = self.pending_shortcut.take() else {
            return Task::none();
        };
        let description = t!(Langs::SHOW_WINDOW, locale = self.language.locale()).into_owned();
        let (registration, task) =
            shortcut::Registration::start(self.wayland, description, binding);
        self.registration = Some(registration);
        task.map(Message::Shortcut)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced_window::events().filter_map(|(id, event)| match event {
                iced_window::Event::CloseRequested => Some(Message::CloseRequested(id)),
                iced_window::Event::Closed => Some(Message::WindowClosed(id)),
                iced_window::Event::Resized(size) => Some(Message::WindowResized(id, size)),
                iced_window::Event::Focused => Some(Message::WindowFocused(id, true)),
                iced_window::Event::Unfocused => Some(Message::WindowFocused(id, false)),
                _ => None,
            }),
            event::listen_with(Self::keyboard_shortcut),
        ])
    }

    fn keyboard_shortcut(
        event: Event,
        status: event::Status,
        window: iced_window::Id,
    ) -> Option<Message> {
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return None;
        };

        match key.as_ref() {
            // 让控件先处理自身的取消行为；剩余 Esc 与窗口关闭按钮共用常驻逻辑。
            keyboard::Key::Named(keyboard::key::Named::Escape)
                if modifiers.is_empty() && status == event::Status::Ignored =>
            {
                Some(Message::CloseRequested(window))
            }
            keyboard::Key::Character("q") if modifiers.command() => Some(Message::Exit),
            _ => None,
        }
    }

    pub fn title(&self) -> String {
        t!(Langs::APP_TITLE, locale = self.language.locale()).into_owned()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if self.exiting {
            match message {
                Message::Shortcut(shortcut::Event::Stopped) => self.shortcut_stopped = true,
                Message::Tray(tray::Event::Stopped) => self.tray_stopped = true,
                _ => return Task::none(),
            }
            return if self.shortcut_stopped && self.tray_stopped {
                iced::exit()
            } else {
                Task::none()
            };
        }
        match message {
            #[cfg(target_os = "macos")]
            Message::RequestSelectionPermission => {
                self.cancel_capture("selection permission requested");
                text_capture::macos::permission(true).map(Message::SelectionPermissionChecked)
            }
            #[cfg(target_os = "macos")]
            Message::SelectionPermissionChecked(result) => {
                match result {
                    Ok(true) => {
                        if matches!(
                            self.capture_notice,
                            Some(
                                text_capture::Error::AccessibilityRequired
                                    | text_capture::Error::AccessibilitySettings
                            )
                        ) {
                            self.capture_notice = None;
                        }
                    }
                    Ok(false) => {
                        self.capture_notice = Some(text_capture::Error::AccessibilityRequired)
                    }
                    Err(error) => self.capture_notice = Some(error),
                }
                Task::none()
            }
            Message::StartTray => {
                if self.tray.is_some() {
                    return Task::none();
                }
                let (registration, events) = tray::Registration::start(self.language);
                self.tray = Some(registration);
                events.map(Message::Tray)
            }
            Message::Tray(tray::Event::Ready) => {
                self.tray_ready = true;
                self.tray_failed = false;
                self.close_blocked = false;
                Task::none()
            }
            Message::Tray(tray::Event::Unavailable(error)) => {
                eprintln!("[tray] unavailable: {error}");
                self.tray_ready = false;
                self.tray_failed = true;
                if self.can_hide() {
                    Task::none()
                } else {
                    self.show_window()
                }
            }
            Message::Tray(tray::Event::Show) => {
                self.cancel_capture("tray show");
                self.show_window()
            }
            Message::Tray(tray::Event::Exit) => self.update(Message::Exit),
            Message::Tray(tray::Event::Stopped) => Task::none(),
            Message::ConfigurationLoaded {
                service,
                show_window,
            } => {
                let service = self
                    .panel
                    .update(translation_panel::Message::ServiceLoaded(service))
                    .map(Message::Panel);
                match show_window {
                    Ok(binding) => self.pending_shortcut = Some(binding),
                    Err(_) => self.shortcut_status = ShortcutStatus::Failed,
                }
                Task::batch([service, self.register_shortcut()])
            }
            Message::LanguageChanged(language) => {
                self.language = language;
                if let Some(tray) = &self.tray {
                    tray.set_language(language);
                }
                Task::none()
            }
            Message::Panel(message) => {
                use translation_panel::Message as PanelMessage;
                let changes_input = matches!(&message, PanelMessage::InputAction(action) if action.is_edit())
                    || matches!(
                        message,
                        PanelMessage::Clear | PanelMessage::SwapLanguages | PanelMessage::Translate
                    );
                if changes_input {
                    self.cancel_capture("input action or submit");
                }
                self.panel.update(message).map(Message::Panel)
            }
            Message::WindowOpened(id) => iced_window::run(id, |window| {
                use iced_window::raw_window_handle::RawWindowHandle;
                matches!(
                    window.window_handle().map(|h| h.as_raw()),
                    Ok(RawWindowHandle::Wayland(_))
                )
            })
            .map(move |wayland| Message::WindowReady(id, wayland)),
            Message::WindowReady(id, wayland) => {
                if self.window != WindowState::Opening(id) {
                    return Task::none();
                }
                self.window = WindowState::Visible(id);
                self.wayland = wayland;
                let focus = Self::focus_input(id);
                let baseline = if !self.capture_baseline_started {
                    self.capture_baseline_started = true;
                    text_capture::read(wayland).map(Message::CaptureBaseline)
                } else {
                    Task::none()
                };
                Task::batch([focus, self.register_shortcut(), baseline])
            }
            Message::CloseRequested(id) => self.hide_window(id),
            Message::WindowClosed(id) => self.window_closed(id),
            Message::WindowResized(id, size) => {
                if self.window.id() == Some(id) && size.width > 0.0 && size.height > 0.0 {
                    self.window_size = size;
                }
                Task::none()
            }
            Message::WindowFocused(id, focused) => {
                if self.window.id() == Some(id) {
                    self.focused = focused;
                    #[cfg(target_os = "macos")]
                    if focused && self.capture.is_none() {
                        return text_capture::macos::permission(false)
                            .map(Message::SelectionPermissionChecked);
                    }
                    return self.read_pending_capture();
                }
                Task::none()
            }
            Message::Shortcut(shortcut::Event::Ready(description)) => {
                // Portal 注册成功与按键描述是否存在是两件事。Hyprland 可能
                // 返回空描述，不能据此阻止关闭或要求用户先触发一次热键。
                self.shortcut_status = ShortcutStatus::Ready(description);
                self.close_blocked = false;
                Task::none()
            }
            Message::Shortcut(shortcut::Event::Activated) => self.show_with_text(),
            Message::CaptureBaseline(snapshot) => {
                if self.capture_revision == 0 {
                    eprintln!("[capture] startup baseline: {snapshot:?}");
                    self.capture_snapshot = Some(snapshot);
                }
                Task::none()
            }
            Message::TextCaptured { revision, snapshot } => self.text_captured(revision, snapshot),
            Message::Shortcut(shortcut::Event::Failed(error)) => {
                eprintln!("Global shortcut unavailable: {error}");
                self.shortcut_status = ShortcutStatus::Failed;
                // 两个恢复入口都失效时，必须恢复窗口，避免后台进程无法操作。
                if self.can_hide() {
                    Task::none()
                } else {
                    self.show_window()
                }
            }
            Message::Exit => {
                self.cancel_capture("exit");
                self.exiting = true;
                let shortcut = match self.registration.take() {
                    Some(registration) => registration.stop().map(Message::Shortcut),
                    None => Task::done(Message::Shortcut(shortcut::Event::Stopped)),
                };
                let tray = match self.tray.take() {
                    Some(registration) => registration.stop().map(Message::Tray),
                    None => Task::done(Message::Tray(tray::Event::Stopped)),
                };
                Task::batch([shortcut, tray])
            }
            Message::Shortcut(shortcut::Event::Stopped) => Task::none(),
        }
    }
}
