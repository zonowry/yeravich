use std::{collections::HashMap, future::Future, hash::Hash};

use futures_util::future::{AbortHandle, Abortable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(u64);

/// Owns cancellation independently of the executor that polls each future.
pub struct TaskScope<K> {
    next_id: u64,
    tasks: HashMap<K, (TaskId, AbortHandle)>,
}

impl<K> Default for TaskScope<K> {
    fn default() -> Self {
        Self {
            next_id: 0,
            tasks: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash> TaskScope<K> {
    pub fn cancel(&mut self, key: &K) {
        if let Some((_, task)) = self.tasks.remove(key) {
            task.abort();
        }
    }

    pub fn start<F: Future>(&mut self, key: K, future: F) -> (TaskId, Abortable<F>) {
        self.next_id = self.next_id.checked_add(1).expect("task IDs exhausted");
        let id = TaskId(self.next_id);
        let (abort, registration) = AbortHandle::new_pair();
        if let Some((_, previous)) = self.tasks.insert(key, (id, abort)) {
            previous.abort();
        }
        (id, Abortable::new(future, registration))
    }

    /// Rejects completions already queued by a task that has since been replaced.
    pub fn finish(&mut self, key: &K, id: TaskId) -> bool {
        if self.tasks.get(key).is_some_and(|(active, _)| *active == id) {
            self.tasks.remove(key);
            true
        } else {
            false
        }
    }
}

impl<K> Drop for TaskScope<K> {
    fn drop(&mut self) {
        for (_, task) in self.tasks.drain().map(|(_, task)| task) {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use futures_executor::block_on;
    use futures_util::{FutureExt, future::Aborted};

    use super::TaskScope;

    struct DropSignal(Arc<AtomicBool>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn replacing_running_work_drops_its_resources() {
        let dropped = Arc::new(AtomicBool::new(false));
        let signal = DropSignal(Arc::clone(&dropped));
        let mut scope = TaskScope::default();
        let (old_id, old) = scope.start("translate", async move {
            let _signal = signal;
            pending::<()>().await;
        });
        let mut old = Box::pin(old);
        assert!(old.as_mut().now_or_never().is_none());
        let (new_id, new) = scope.start("translate", async { 42 });

        assert_eq!(block_on(old), Err(Aborted));
        assert!(dropped.load(Ordering::SeqCst));
        assert!(!scope.finish(&"translate", old_id));
        assert_eq!(block_on(new), Ok(42));
        assert!(scope.finish(&"translate", new_id));
        assert!(!scope.finish(&"translate", new_id));
    }

    #[test]
    fn dropping_scope_cancels_every_operation() {
        let mut scope = TaskScope::default();
        let (_, translation) = scope.start("translate", pending::<()>());
        let (_, capture) = scope.start("capture", pending::<()>());

        drop(scope);

        assert_eq!(block_on(translation), Err(Aborted));
        assert_eq!(block_on(capture), Err(Aborted));
    }

    #[test]
    fn replacing_one_operation_leaves_other_work_running() {
        let mut scope = TaskScope::default();
        let (_, translation) = scope.start("translate", pending::<()>());
        let (capture_id, capture) = scope.start("capture", async { "captured" });
        let (_, replacement) = scope.start("translate", async {});

        assert_eq!(block_on(translation), Err(Aborted));
        assert_eq!(block_on(capture), Ok("captured"));
        assert!(scope.finish(&"capture", capture_id));
        assert_eq!(block_on(replacement), Ok(()));
    }
}
