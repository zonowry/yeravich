use iced::Task;
use iced::widget::text_editor;

use super::{Message, ServiceState, TranslationPanel};
use crate::translation;
impl TranslationPanel {
    pub(in crate::app) fn import_and_translate(&mut self, text: &str) -> Task<Message> {
        if text.trim().is_empty() {
            eprintln!("[translation] skipped: empty captured text");
            return Task::none();
        }
        eprintln!(
            "[translation] captured bytes={} changed={}",
            text.len(),
            self.input.text() != text
        );
        self.import_text(text);
        // 只复用完整结果，失败后保留的部分译文不能阻止重试。
        if !self.translating && self.error.is_none() && !self.output.is_empty() {
            eprintln!(
                "[translation] reused completed result revision={}",
                self.revision
            );
            return Task::none();
        }
        // 读取文本可能先于配置加载完成；只为这次带入保留自动提交意图。
        self.auto_translate_pending = matches!(self.service, ServiceState::Loading);
        if self.auto_translate_pending {
            eprintln!("[translation] waiting for configuration");
        }
        self.update(Message::Translate)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ServiceLoaded(result) => {
                eprintln!("[translation] service ready={}", result.is_ok());
                self.service = match result {
                    Ok(service) => ServiceState::Ready(service),
                    Err(error) => ServiceState::Failed(error),
                };
                if self.auto_translate_pending {
                    self.auto_translate_pending = false;
                    return self.update(Message::Translate);
                }
            }
            Message::InputAction(action) => {
                let edited = action.is_edit();
                self.input.perform(action);
                if edited {
                    self.invalidate_translation();
                }
            }
            Message::OutputAction(action) => {
                // Keep selection, scrolling and copying available in the read-only result.
                if !action.is_edit() {
                    self.output.perform(action);
                }
            }
            Message::SwapLanguages => {
                self.source_language = self.source_language.other();
                self.invalidate_translation();
            }
            Message::Clear => {
                self.input = text_editor::Content::new();
                self.invalidate_translation();
            }
            Message::Copy => {
                if !self.output.is_empty() {
                    return iced::clipboard::write(self.output.text());
                }
            }
            Message::Translate => {
                if self.can_translate() {
                    let ServiceState::Ready(service) = &self.service else {
                        return Task::none();
                    };
                    // 任务持有配置快照和 Client 句柄；Client::clone 复用连接池。
                    let service = service.clone();
                    let input = self.input.text();
                    let source = self.source_language;
                    self.invalidate_translation();
                    self.translating = true;
                    let revision = self.revision;
                    eprintln!(
                        "[translation #{revision}] request started bytes={}",
                        input.len()
                    );

                    let started = std::time::Instant::now();
                    let (task, handle) = Task::run(
                        service.translate(input, source, source.other()),
                        move |event| match event {
                            translation::Event::Delta(delta) => Message::TranslationDelta { revision, delta },
                            translation::Event::Finished(result) => {
                                eprintln!(
                                    "[translation #{revision}] request finished elapsed_ms={} result={:?}",
                                    started.elapsed().as_millis(),
                                    result
                                );
                                Message::Translated { revision, result }
                            }
                        },
                    )
                    .abortable();
                    self.translation_task = Some(handle.abort_on_drop());
                    return task;
                } else {
                    eprintln!(
                        "[translation] skipped: service_ready={} in_flight={} empty_input={}",
                        matches!(self.service, ServiceState::Ready(_)),
                        self.translating,
                        self.input.text().trim().is_empty()
                    );
                }
            }
            Message::TranslationDelta { revision, delta } => {
                if revision == self.revision && self.translating && !delta.is_empty() {
                    // 在末尾追加，保留用户的选择；不要每个片段都重建整个编辑器。
                    let cursor = self.output.cursor();
                    self.output
                        .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
                    self.output
                        .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                            delta.into(),
                        )));
                    self.output.move_to(cursor);
                }
            }
            Message::Translated { revision, result } => {
                // 每个片段、完成和错误都只属于发起它的输入版本。
                if revision == self.revision && self.translating {
                    self.translating = false;
                    self.translation_task = None;
                    self.error = result.err();
                } else {
                    eprintln!(
                        "[translation #{revision}] ignored stale response; current={}",
                        self.revision
                    );
                }
            }
        }

        Task::none()
    }
}
