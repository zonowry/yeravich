use std::fmt;

use futures_util::future::Abortable;
use yeravich_core::{
    CoreFuture, SecretValue,
    openai::{ChatSettings, ConnectionReport},
    settings::{SecretChange, SettingsError, SettingsService},
};
use zeroize::Zeroizing;

use crate::task_scope::{TaskId, TaskScope};

/// Editable secret text never appears in iced message diagnostics.
#[derive(Clone, Default)]
pub struct SecretInput(Zeroizing<String>);

impl SecretInput {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretInput([redacted])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Testing,
    Saving,
}

pub struct ViewState {
    pub address: String,
    pub model: String,
    pub key: SecretInput,
    pub has_saved_key: bool,
    pub remove_key: bool,
    pub activity: Activity,
    pub status: String,
    pub error: bool,
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Tested(Result<ConnectionReport, SettingsError>),
    Saved(Result<(), SettingsError>),
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub id: TaskId,
    pub outcome: Outcome,
}

pub struct PendingAction {
    pub id: TaskId,
    pub future: Abortable<CoreFuture<Outcome>>,
}

pub struct SettingsController {
    service: SettingsService,
    state: ViewState,
    tasks: TaskScope<()>,
}

impl SettingsController {
    pub fn new(service: SettingsService) -> Self {
        let profile = service.saved_profile();
        Self {
            state: ViewState {
                address: profile
                    .as_ref()
                    .and_then(|p| p.public.get("base_url"))
                    .cloned()
                    .unwrap_or_else(|| "https://api.openai.com/v1".into()),
                model: profile
                    .as_ref()
                    .and_then(|p| p.public.get("model"))
                    .cloned()
                    .unwrap_or_default(),
                key: SecretInput::default(),
                has_saved_key: profile.is_some_and(|profile| profile.secret_ref.is_some()),
                remove_key: false,
                activity: Activity::Idle,
                status: String::new(),
                error: false,
            },
            service,
            tasks: TaskScope::default(),
        }
    }

    pub fn state(&self) -> &ViewState {
        &self.state
    }

    fn edit(&mut self) -> bool {
        if self.state.activity == Activity::Saving {
            return false;
        }
        self.tasks.cancel(&());
        self.state.activity = Activity::Idle;
        self.state.status = "Unsaved changes".into();
        self.state.error = false;
        true
    }

    pub fn set_address(&mut self, value: String) {
        if self.edit() {
            self.state.address = value;
        }
    }

    pub fn set_model(&mut self, value: String) {
        if self.edit() {
            self.state.model = value;
        }
    }

    pub fn set_key(&mut self, value: SecretInput) {
        if self.edit() {
            self.state.key = value;
            self.state.remove_key = false;
        }
    }

    pub fn remove_key(&mut self, remove: bool) {
        if self.edit() {
            self.state.remove_key = remove;
            if remove {
                self.state.key = SecretInput::default();
            }
        }
    }

    pub fn leave(&mut self) {
        if self.state.activity == Activity::Testing {
            self.tasks.cancel(&());
            self.state.activity = Activity::Idle;
            self.state.status = "Connection test cancelled".into();
        }
    }

    fn draft(&self) -> Result<(ChatSettings, SecretChange), SettingsError> {
        let settings = ChatSettings::new(&self.state.address, &self.state.model)?;
        let key = self.state.key.as_str().trim();
        let change = if self.state.remove_key {
            SecretChange::Remove
        } else if key.is_empty() {
            SecretChange::Keep
        } else {
            SecretChange::Replace(SecretValue::new(key.as_bytes().to_vec()))
        };
        Ok((settings, change))
    }

    pub fn start(&mut self, activity: Activity) -> Option<PendingAction> {
        if self.state.activity == Activity::Saving || activity == Activity::Idle {
            return None;
        }
        self.tasks.cancel(&());
        self.state.activity = Activity::Idle;
        let (settings, change) = match self.draft() {
            Ok(draft) => draft,
            Err(error) => {
                self.state.error = true;
                self.state.status = error.to_string();
                return None;
            }
        };
        self.state.activity = activity;
        self.state.error = false;
        self.state.status = if activity == Activity::Testing {
            "Testing connection…"
        } else {
            "Saving…"
        }
        .into();
        let service = self.service.clone();
        let future: CoreFuture<_> = Box::pin(async move {
            if activity == Activity::Testing {
                Outcome::Tested(service.test_connection(settings, change).await)
            } else {
                Outcome::Saved(service.save(settings, change).await)
            }
        });
        let (id, future) = self.tasks.start((), future);
        Some(PendingAction { id, future })
    }

    pub fn complete(&mut self, completion: Completion) {
        if !self.tasks.finish(&(), completion.id) {
            return;
        }
        self.state.activity = Activity::Idle;
        self.state.error = false;
        match completion.outcome {
            Outcome::Tested(Ok(report)) => {
                self.state.status = format!(
                    "Connected successfully ({} ms). Settings have not been saved by this test.",
                    report.elapsed.as_millis()
                );
            }
            Outcome::Saved(Ok(())) => {
                self.state.has_saved_key = self
                    .service
                    .saved_profile()
                    .is_some_and(|profile| profile.secret_ref.is_some());
                self.state.key = SecretInput::default();
                self.state.remove_key = false;
                self.state.status = "Settings saved".into();
            }
            Outcome::Tested(Err(error)) | Outcome::Saved(Err(error)) => {
                self.state.error = true;
                self.state.status = error.to_string();
            }
        }
    }
}

#[cfg(test)]
mod tests;
