use iced::Task;
use iced::futures::{
    FutureExt, StreamExt,
    channel::{mpsc, oneshot},
    pin_mut, select,
};
use ksni::TrayMethods;

use super::{Event, ICON_RGBA, ICON_SIZE, Labels};
use crate::i18n::Language;

pub struct Registration {
    shutdown: oneshot::Sender<()>,
    language: mpsc::UnboundedSender<Language>,
}

impl Registration {
    pub fn start(language: Language) -> (Self, Task<Event>) {
        let (shutdown, receive_shutdown) = oneshot::channel();
        let (send_language, receive_language) = mpsc::unbounded();
        let (events, receive_events) = mpsc::unbounded();
        let tray = Tray {
            labels: Labels::new(language),
            events,
        };
        (
            Self {
                shutdown,
                language: send_language,
            },
            Task::batch([
                Task::run(receive_events, std::convert::identity),
                Task::perform(run(tray, receive_shutdown, receive_language), |_| {
                    Event::Stopped
                }),
            ]),
        )
    }

    pub fn set_language(&self, language: Language) {
        let _ = self.language.unbounded_send(language);
    }

    pub fn stop(self) -> Task<Event> {
        if self.shutdown.send(()).is_ok() {
            Task::none()
        } else {
            // 初始化失败或服务已经停止时，没有后台清理需要等待。
            Task::done(Event::Stopped)
        }
    }
}

async fn run(
    tray: Tray,
    shutdown: oneshot::Receiver<()>,
    mut language: mpsc::UnboundedReceiver<Language>,
) {
    let events = tray.events.clone();
    let setup = tray.spawn().fuse();
    let shutdown = shutdown.fuse();
    pin_mut!(setup, shutdown);
    // D-Bus 初始化不阻塞窗口线程；用户在初始化期间退出也能取消等待。
    let handle = select! {
        result = setup => match result {
            Ok(handle) => handle,
            Err(error) => {
                let _ = events.unbounded_send(Event::Unavailable(error.to_string()));
                return;
            }
        },
        _ = shutdown => return,
    };
    let _ = events.unbounded_send(Event::Ready);
    loop {
        select! {
            _ = shutdown => break,
            next = language.next().fuse() => {
                let Some(language) = next else { break };
                if handle.update(|tray| tray.labels = Labels::new(language)).await.is_none() {
                    let _ = events.unbounded_send(Event::Unavailable("Tray service ended".into()));
                    break;
                }
            }
        }
    }
    handle.shutdown().await;
}

struct Tray {
    labels: Labels,
    events: mpsc::UnboundedSender<Event>,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "com.zonowry.yeravich".into()
    }

    fn title(&self) -> String {
        "Yeravich".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Yeravich".into(),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: ICON_SIZE as i32,
            height: ICON_SIZE as i32,
            // StatusNotifierItem 使用网络字节序 ARGB，原始资源是 RGBA。
            data: ICON_RGBA
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
                .collect(),
        }]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.events.unbounded_send(Event::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: self.labels.show.clone(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.events.unbounded_send(Event::Show);
                }),
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: self.labels.exit.clone(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.events.unbounded_send(Event::Exit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        let _ = self
            .events
            .unbounded_send(Event::Unavailable(format!("{reason:?}")));
        // 面板重启时等待它恢复；由 App 决定是否需要恢复窗口。
        true
    }

    fn watcher_online(&self) {
        let _ = self.events.unbounded_send(Event::Ready);
    }
}
