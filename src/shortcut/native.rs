use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use iced::futures::channel::mpsc;

use super::{Binding, Event};

pub(super) struct Registration {
    manager: GlobalHotKeyManager,
    hotkey: HotKey,
}

impl Registration {
    // Windows 和 macOS 要求注册与窗口事件循环处于同一线程。
    // 因此从 App::update 调用，而不是放入异步任务中。
    pub fn new(binding: Binding) -> global_hotkey::Result<(Self, mpsc::UnboundedReceiver<Event>)> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = binding.hotkey();
        let (sender, receiver) = mpsc::unbounded();

        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == hotkey.id() && event.state == HotKeyState::Pressed {
                let _ = sender.unbounded_send(Event::Activated);
            }
        }));
        manager.register(hotkey)?;

        Ok((Self { manager, hotkey }, receiver))
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Err(error) = self.manager.unregister(self.hotkey) {
            eprintln!("Could not unregister the global shortcut: {error}");
        }
    }
}
