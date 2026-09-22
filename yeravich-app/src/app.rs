use std::{fmt, sync::Arc};

use iced::{
    Element, Fill, Task,
    widget::{button, column, container, row, text},
};
use yeravich_core::{
    AppConfig, Yeravich,
    openai::{ADAPTER_ID, ChatClient},
    settings::{SettingsError, SettingsService},
};

use crate::{
    pages::{settings, translation},
    platform::{config::FileConfigStore, secrets::FileSecretStore},
    task_scope::{TaskId, TaskScope},
};

pub struct App {
    workspace: Option<Workspace>,
    startup_error: Option<SettingsError>,
    tasks: TaskScope<()>,
}

struct Workspace {
    screen: Screen,
    translation: translation::Page,
    settings: settings::Page,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Translation,
    Settings,
}

#[derive(Clone)]
pub struct Services {
    core: Yeravich,
    settings: SettingsService,
}

#[derive(Clone)]
pub enum Message {
    Loaded(TaskId, Result<Services, SettingsError>),
    Navigate(Screen),
    Translation(translation::Message),
    Settings(settings::Message),
}

impl fmt::Debug for Message {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AppMessage([redacted])")
    }
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let mut app = Self {
            workspace: None,
            startup_error: None,
            tasks: TaskScope::default(),
        };
        let (id, future) = app.tasks.start((), async {
            let core = Yeravich::new(AppConfig::default(), Arc::new(FileSecretStore::new()?));
            let client = ChatClient::new()?;
            core.register_backend(ADAPTER_ID, Arc::new(client.clone()));
            let settings =
                SettingsService::new(core.clone(), Arc::new(FileConfigStore::new()?), client);
            settings.load().await?;
            Ok(Services { core, settings })
        });
        let task = Task::perform(future, move |result| {
            result.ok().map(|result| Message::Loaded(id, result))
        })
        .and_then(Task::done);
        (app, task)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(id, result) => {
                if self.tasks.finish(&(), id) {
                    match result {
                        Ok(services) => {
                            self.workspace = Some(Workspace {
                                screen: Screen::Translation,
                                translation: translation::Page::new(services.core),
                                settings: settings::Page::new(services.settings),
                            });
                        }
                        Err(error) => self.startup_error = Some(error),
                    }
                }
            }
            Message::Navigate(screen) => {
                if let Some(workspace) = &mut self.workspace
                    && !workspace.settings.is_saving()
                {
                    if screen != workspace.screen {
                        workspace.settings.leave();
                    }
                    workspace.screen = screen;
                }
            }
            Message::Translation(message) => {
                if let Some(workspace) = &mut self.workspace {
                    return workspace
                        .translation
                        .update(message)
                        .map(Message::Translation);
                }
            }
            Message::Settings(message) => {
                if let Some(workspace) = &mut self.workspace {
                    return workspace.settings.update(message).map(Message::Settings);
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let Some(workspace) = &self.workspace else {
            let status = self.startup_error.as_ref().map_or_else(
                || "Loading settings…".into(),
                |error| format!("{error}\nCheck the configuration file and restart Yeravich."),
            );
            return container(text(status)).padding(24).into();
        };
        let navigation = row![
            button("Translate")
                .style(if workspace.screen == Screen::Translation {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press_maybe(
                    (!workspace.settings.is_saving())
                        .then_some(Message::Navigate(Screen::Translation))
                ),
            button("Settings")
                .style(if workspace.screen == Screen::Settings {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press_maybe(
                    (!workspace.settings.is_saving())
                        .then_some(Message::Navigate(Screen::Settings))
                ),
        ]
        .spacing(8)
        .padding([12, 24]);
        let page = match workspace.screen {
            Screen::Translation => workspace.translation.view().map(Message::Translation),
            Screen::Settings => workspace.settings.view().map(Message::Settings),
        };
        column![navigation, page].height(Fill).into()
    }
}
