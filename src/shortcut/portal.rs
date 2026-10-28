use ashpd::desktop::Session;
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use iced::Task;
use iced::futures::{
    FutureExt, SinkExt, StreamExt,
    channel::{mpsc, oneshot},
    pin_mut, select,
};

use super::Event;

const SHORTCUT_ID: &str = "show";

pub(super) fn listen(
    description: String,
    preferred_trigger: String,
) -> (oneshot::Sender<()>, Task<Event>) {
    let (shutdown, receive_shutdown) = oneshot::channel();
    let task = Task::run(
        iced::stream::channel(16, move |mut output| async move {
            if let Err(error) = run(
                &mut output,
                description,
                preferred_trigger,
                receive_shutdown,
            )
            .await
            {
                let _ = output.send(Event::Failed(error.to_string())).await;
            }
            let _ = output.send(Event::Stopped).await;
        }),
        std::convert::identity,
    );
    (shutdown, task)
}

async fn run(
    output: &mut mpsc::Sender<Event>,
    description: String,
    preferred_trigger: String,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let setup = async {
        let connection = ashpd::zbus::connection::Builder::session()?
            .method_timeout(std::time::Duration::from_secs(3))
            .build()
            .await?;
        ashpd::register_host_app_with_connection(
            connection.clone(),
            "com.zonowry.yeravich".parse()?,
        )
        .await?;
        // 独立连接只有这一组快捷键会话，信号不会混入应用的其他 Portal 会话。
        let portal = GlobalShortcuts::with_connection(connection).await?;
        let session = portal.create_session(Default::default()).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((portal, session))
    }
    .fuse();
    let shutdown = shutdown.fuse();
    pin_mut!(setup, shutdown);
    let (portal, session) = select! {
        result = setup => result?,
        _ = shutdown => return Ok(()),
    };

    let result = {
        let listening =
            listen_session(output, description, &preferred_trigger, &portal, &session).fuse();
        pin_mut!(listening);
        select! {
            result = listening => result,
            _ = shutdown => Ok(()),
        }
    };
    // 先关闭会话再通知 App 退出，也覆盖用户仍在快捷键授权对话框中的情况。
    session.close().await?;
    result
}

async fn listen_session(
    output: &mut mpsc::Sender<Event>,
    description: String,
    preferred_trigger: &str,
    portal: &GlobalShortcuts,
    session: &Session<GlobalShortcuts>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 先订阅再绑定，避免遗漏绑定完成后立即发生的事件。
    let mut activated = portal.receive_activated().await?;
    let mut changed = portal.receive_shortcuts_changed().await?;
    let mut closed = session.receive_closed().await?;
    let mut owner_changed = portal.receive_owner_changed().await?;
    let binding = portal
        .bind_shortcuts(
            session,
            &[NewShortcut::new(SHORTCUT_ID, description).preferred_trigger(preferred_trigger)],
            None,
            Default::default(),
        )
        .await?
        .response()?;

    let Some(shortcut) = binding.shortcuts().iter().find(|s| s.id() == SHORTCUT_ID) else {
        return Err("The desktop did not grant a shortcut".into());
    };
    output
        .send(Event::Ready(shortcut.trigger_description().into()))
        .await?;

    loop {
        select! {
            event = activated.next().fuse() => {
                let Some(event) = event else { break };
                if event.shortcut_id() == SHORTCUT_ID {
                    output.send(Event::Activated).await?;
                }
            }
            event = changed.next().fuse() => {
                let Some(event) = event else { break };
                if let Some(shortcut) = event.shortcuts().iter().find(|s| s.id() == SHORTCUT_ID) {
                    output.send(Event::Ready(shortcut.trigger_description().into())).await?;
                } else {
                    break;
                }
            }
            _ = closed.next().fuse() => break,
            _ = owner_changed.next().fuse() => break,
        }
    }

    Err("The desktop shortcut session ended".into())
}
