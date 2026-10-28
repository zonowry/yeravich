use iced::Task;
use iced::futures::channel::mpsc;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

use super::{Event, ICON_RGBA, ICON_SIZE, Labels};
use crate::i18n::Language;

pub struct Registration {
    _icon: Option<TrayIcon>,
    show: Option<MenuItem>,
    exit: Option<MenuItem>,
}

impl Registration {
    // 原生托盘和菜单不是 Send，必须在已经运行的窗口事件线程创建和释放。
    pub fn start(language: Language) -> (Self, Task<Event>) {
        match Self::new(language) {
            Ok((registration, events)) => (
                registration,
                Task::done(Event::Ready).chain(Task::run(events, std::convert::identity)),
            ),
            Err(error) => (
                Self {
                    _icon: None,
                    show: None,
                    exit: None,
                },
                Task::done(Event::Unavailable(error.to_string())),
            ),
        }
    }

    fn new(
        language: Language,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Event>), Box<dyn std::error::Error>> {
        let labels = Labels::new(language);
        let show = MenuItem::new(labels.show, true, None);
        let exit = MenuItem::new(labels.exit, true, None);
        let menu = Menu::with_items(&[&show, &exit])?;
        let icon = Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE)?;
        let builder = TrayIconBuilder::new()
            .with_id("com.zonowry.yeravich")
            .with_icon(icon)
            .with_tooltip("Yeravich")
            .with_menu(Box::new(menu))
            // macOS 保持原生左键菜单习惯；Windows 左键显示，右键打开菜单。
            .with_menu_on_left_click(cfg!(target_os = "macos"));

        // tray-icon 的回调只能初始化一次，必须先于 build 可能产生的系统事件。
        // App 只创建一次注册；回调持有发送端直到进程退出，不尝试重新设置。
        let (sender, receiver) = mpsc::unbounded();
        let show_id = show.id().clone();
        let exit_id = exit.id().clone();
        let menu_sender = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id == show_id {
                Event::Show
            } else if event.id == exit_id {
                Event::Exit
            } else {
                return;
            };
            let _ = menu_sender.unbounded_send(action);
        }));
        let icon_id = builder.id().clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if cfg!(target_os = "windows")
                && matches!(event, TrayIconEvent::Click {
                    id, button: MouseButton::Left, button_state: MouseButtonState::Up, ..
                } if id == icon_id)
            {
                let _ = sender.unbounded_send(Event::Show);
            }
        }));
        let icon = builder.build()?;
        Ok((
            Self {
                _icon: Some(icon),
                show: Some(show),
                exit: Some(exit),
            },
            receiver,
        ))
    }

    pub fn set_language(&self, language: Language) {
        let labels = Labels::new(language);
        if let Some(show) = &self.show {
            show.set_text(labels.show);
        }
        if let Some(exit) = &self.exit {
            exit.set_text(labels.exit);
        }
    }

    pub fn stop(self) -> Task<Event> {
        // 丢弃最后一个 TrayIcon，在窗口事件线程移除系统图标。
        drop(self);
        Task::done(Event::Stopped)
    }
}
