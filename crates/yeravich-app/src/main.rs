mod task_scope;

use std::{cell::RefCell, rc::Rc, sync::Arc};

use slint::{ComponentHandle, SharedString};
use task_scope::TaskScope;
use yeravich_core::{
    AppConfig, CoreFuture, SecretReference, SecretStore, SecretStoreError, SecretValue, Yeravich,
};

slint::include_modules!();

#[derive(Debug, Clone)]
struct ViewState {
    source_language: String,
    target_language: String,
    source_text: String,
    translated_text: String,
    status: String,
    busy: bool,
}

impl ViewState {
    fn from_core(core: &Yeravich) -> Self {
        let config = core.config();
        Self {
            source_language: config.languages.source,
            target_language: config.languages.target,
            source_text: String::new(),
            translated_text: String::new(),
            status: "Ready".into(),
            busy: false,
        }
    }

    fn as_ui_state(&self) -> UiState {
        UiState {
            source_language: SharedString::from(&self.source_language),
            target_language: SharedString::from(&self.target_language),
            source_text: SharedString::from(&self.source_text),
            translated_text: SharedString::from(&self.translated_text),
            status: SharedString::from(&self.status),
            busy: self.busy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Operation {
    Translation,
}

struct UnavailableSecretStore;

impl SecretStore for UnavailableSecretStore {
    fn load(
        &self,
        _reference: &SecretReference,
    ) -> CoreFuture<Result<SecretValue, SecretStoreError>> {
        Box::pin(async { Err(SecretStoreError::Unavailable) })
    }
}

fn publish(ui: &slint::Weak<MainWindow>, state: &ViewState) {
    if let Some(ui) = ui.upgrade() {
        ui.set_state(state.as_ui_state());
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let core = Yeravich::new(AppConfig::default(), Arc::new(UnavailableSecretStore));
    let state = Rc::new(RefCell::new(ViewState::from_core(&core)));
    let tasks = Rc::new(TaskScope::default());
    let ui = MainWindow::new()?;
    let ui_weak = ui.as_weak();

    ui.set_state(state.borrow().as_ui_state());

    {
        let state = Rc::clone(&state);
        let weak = ui_weak.clone();
        ui.on_source_edited(move |text| {
            state.borrow_mut().source_text = text.into();
            publish(&weak, &state.borrow());
        });
    }

    {
        let core = core.clone();
        let state = Rc::clone(&state);
        let weak = ui_weak.clone();
        ui.on_swap_languages(move || {
            core.swap_languages();
            let config = core.config();
            let mut state = state.borrow_mut();
            state.source_language = config.languages.source;
            state.target_language = config.languages.target;
            publish(&weak, &state);
        });
    }

    {
        let core = core.clone();
        let state = Rc::clone(&state);
        let tasks = Rc::clone(&tasks);
        let weak = ui_weak.clone();
        ui.on_translate_requested(move || {
            let source = state.borrow().source_text.clone();
            {
                let mut state = state.borrow_mut();
                state.busy = true;
                state.status = "Translating…".into();
                state.translated_text.clear();
                publish(&weak, &state);
            }

            let core = core.clone();
            let state = Rc::clone(&state);
            let weak = weak.clone();
            tasks.spawn(Operation::Translation, async move {
                let result = core.translate_text(source).await;
                let mut state = state.borrow_mut();
                state.busy = false;
                match result {
                    Ok(translation) => {
                        state.translated_text = translation.text;
                        state.status = "Done".into();
                    }
                    Err(error) => {
                        state.status = error.to_string();
                    }
                }
                publish(&weak, &state);
            });
        });
    }

    let result = ui.run();
    drop(tasks);
    result
}
