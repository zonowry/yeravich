use futures_util::future::Abortable;
use yeravich_core::{CoreFuture, LanguagePair, Translation, TranslationError, Yeravich};

use crate::task_scope::{TaskId, TaskScope};

pub struct ViewState {
    pub languages: LanguagePair,
    pub source_text: String,
    pub translated_text: String,
    pub status: String,
    pub busy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Operation {
    Translation,
}

pub struct PendingTranslation {
    pub id: TaskId,
    pub future: Abortable<CoreFuture<Result<Translation, TranslationError>>>,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub id: TaskId,
    pub result: Result<Translation, TranslationError>,
}

pub struct TranslationController {
    core: Yeravich,
    state: ViewState,
    tasks: TaskScope<Operation>,
}

impl TranslationController {
    pub fn new(core: Yeravich) -> Self {
        Self {
            state: ViewState {
                languages: core.config().languages,
                source_text: String::new(),
                translated_text: String::new(),
                status: "Ready".into(),
                busy: false,
            },
            core,
            tasks: TaskScope::default(),
        }
    }

    pub fn state(&self) -> &ViewState {
        &self.state
    }

    pub fn set_source(&mut self, source: String) {
        if !self.state.busy {
            self.state.source_text = source;
        }
    }

    pub fn swap_languages(&mut self) {
        if !self.state.busy {
            self.core.swap_languages();
            self.state.languages = self.core.config().languages;
        }
    }

    pub fn translate(&mut self) -> Option<PendingTranslation> {
        if self.state.source_text.trim().is_empty() {
            return None;
        }

        self.state.busy = true;
        self.state.status = "Translating…".into();
        self.state.translated_text.clear();

        let core = self.core.clone();
        let source = self.state.source_text.clone();
        let future: CoreFuture<_> = Box::pin(async move { core.translate_text(source).await });
        let (id, future) = self.tasks.start(Operation::Translation, future);
        Some(PendingTranslation { id, future })
    }

    pub fn complete(&mut self, completion: Completion) {
        if !self.tasks.finish(&Operation::Translation, completion.id) {
            return;
        }
        self.state.busy = false;
        match completion.result {
            Ok(translation) => {
                self.state.translated_text = translation.text;
                self.state.status = "Done".into();
            }
            Err(error) => self.state.status = error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests;
