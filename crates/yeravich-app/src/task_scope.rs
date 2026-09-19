use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    hash::Hash,
    rc::Rc,
};

use futures_util::future::{AbortHandle, Abortable};

/// Owns one cancellable child task per operation kind.
///
/// Starting a replacement cancels the previous task. Dropping the scope cancels
/// every child, mirroring the ownership rules of structured concurrency.
pub struct TaskScope<K> {
    next_id: Cell<u64>,
    tasks: RefCell<HashMap<K, (u64, AbortHandle)>>,
}

impl<K> Default for TaskScope<K> {
    fn default() -> Self {
        Self {
            next_id: Cell::new(0),
            tasks: RefCell::new(HashMap::new()),
        }
    }
}

impl<K> TaskScope<K>
where
    K: Copy + Eq + Hash + 'static,
{
    pub fn spawn(self: &Rc<Self>, key: K, future: impl Future<Output = ()> + 'static) {
        self.cancel(key);

        let id = self.next_id.get().wrapping_add(1);
        self.next_id.set(id);
        let (abort, registration) = AbortHandle::new_pair();
        self.tasks.borrow_mut().insert(key, (id, abort));

        let owner = Rc::downgrade(self);
        slint::spawn_local(async move {
            let _ = Abortable::new(future, registration).await;
            if let Some(owner) = owner.upgrade() {
                let mut tasks = owner.tasks.borrow_mut();
                if tasks.get(&key).is_some_and(|(active, _)| *active == id) {
                    tasks.remove(&key);
                }
            }
        })
        .expect("Slint event loop is not available");
    }

    fn cancel(&self, key: K) {
        if let Some((_, task)) = self.tasks.borrow_mut().remove(&key) {
            task.abort();
        }
    }
}

impl<K> Drop for TaskScope<K> {
    fn drop(&mut self) {
        for (_, task) in self.tasks.get_mut().drain().map(|(_, task)| task) {
            task.abort();
        }
    }
}
