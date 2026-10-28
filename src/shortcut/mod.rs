//! 系统快捷键只发送事件，窗口和应用状态仍由 Iced 的主事件循环管理。
mod binding;
mod native;

pub use binding::Binding;

#[cfg(target_os = "linux")]
mod portal;

use iced::Task;

#[derive(Debug, Clone)]
pub enum Event {
    Ready(String),
    Activated,
    Failed(String),
    Stopped,
}

// 持有原生注册直到应用退出；Portal 会话则由异步事件任务持有。
#[derive(Default)]
pub struct Registration {
    _native: Option<native::Registration>,
    #[cfg(target_os = "linux")]
    shutdown: Option<iced::futures::channel::oneshot::Sender<()>>,
}

impl Registration {
    pub fn start(wayland: bool, description: String, binding: Binding) -> (Self, Task<Event>) {
        #[cfg(target_os = "linux")]
        if wayland
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
        {
            // Xwayland 的窗口能隐藏，但全局按键仍由 Wayland 桌面管理。
            let (shutdown, task) = portal::listen(description, binding.preferred_trigger());
            return (
                Self {
                    shutdown: Some(shutdown),
                    ..Self::default()
                },
                task,
            );
        }

        #[cfg(not(target_os = "linux"))]
        let _ = (wayland, description);

        match native::Registration::new(binding) {
            Ok((registration, events)) => (
                Self {
                    _native: Some(registration),
                    #[cfg(target_os = "linux")]
                    shutdown: None,
                },
                Task::done(Event::Ready(binding.description()))
                    .chain(Task::run(events, std::convert::identity)),
            ),
            Err(error) => (
                Self::default(),
                Task::done(Event::Failed(error.to_string())),
            ),
        }
    }

    pub fn stop(self) -> Task<Event> {
        #[cfg(target_os = "linux")]
        if let Some(shutdown) = self.shutdown
            && shutdown.send(()).is_ok()
        {
            return Task::none();
        }
        // 原生注册随 self 离开作用域释放；失败或已停止的 Portal 无需再等待。
        Task::done(Event::Stopped)
    }
}
