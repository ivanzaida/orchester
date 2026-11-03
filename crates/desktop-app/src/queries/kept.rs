//! What a page's or a tab's own save read back or stored, shown at once and until its query moves on: the query has
//! it, or read again after it (a change made meanwhile is then what shows). A failed save keeps nothing, so the query's
//! value shows.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  core::{Signal, SignalValue},
  query::QueryHandle,
};
use parking_lot::Mutex;

#[derive(Clone)]
pub struct Kept<T: Clone + PartialEq + Send + Sync + SignalValue + 'static> {
  value: Signal<Option<T>>,
  /// The query's data the value was first shown over, by address; `Some(None)` over no data.
  over: Arc<Mutex<Option<Option<usize>>>>,
}

impl<T: Clone + PartialEq + Send + Sync + SignalValue + 'static> Kept<T> {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      value: ctx.signal(None),
      over: Arc::default(),
    }
  }

  /// Keeps what a save of the tab's read back or stored.
  pub fn keep(&self, value: T) {
    *self.over.lock() = None;
    self.value.set(Some(value));
  }

  /// What the tab shows: the kept value until `query` has it or read again since, then `stored`, the query's value.
  /// Nothing kept is let go while `saving`.
  pub fn over<D, E>(&self, (stored, query): (Option<T>, &QueryHandle<D, E>), saving: bool) -> Option<T> {
    let Some(value) = self.value.get() else {
      return stored;
    };
    let now = query.data().map(|data| Arc::as_ptr(&data) as usize);
    let over = *self.over.lock().get_or_insert(now);
    if !saving && (now != over || stored.as_ref() == Some(&value)) {
      self.value.set(None);
      return stored;
    }
    Some(value)
  }
}
