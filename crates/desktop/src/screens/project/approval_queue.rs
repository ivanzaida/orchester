//! Which waiting request the approval dialog shows ("Approval choices and waiting requests"): one at a
//! time, the oldest first unless the person moved to another with ‹ › or opened one with *Review*.
//! *Decide later* closes the dialog and answers nothing; while a request it put off still waits, the
//! dialog does not open by itself, and a new request only adds to the count. It opens by itself when
//! nothing was put off.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};

use crate::tasks::approvals::ShownApproval;

/// The dialog's state for one project, shared by its status bar, task pages and the dialog.
#[derive(Clone)]
pub struct ApprovalQueue {
  /// The request the person moved to; the oldest when `None` or when it no longer waits.
  selected: Signal<Option<String>>,
  /// The requests that waited when the person chose *Decide later*.
  deferred: Signal<Vec<String>>,
  /// The request whose *Always allow* step is open.
  always: Signal<Option<String>>,
  /// Identity, for comparing props that hold the queue.
  id: Arc<()>,
}

impl PartialEq for ApprovalQueue {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.id, &other.id)
  }
}

impl lurq::app::component::DevtoolsInspectable for ApprovalQueue {}

/// The request the dialog shows, where it stands among the waiting ones, and its neighbours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuePlace {
  pub index: usize,
  pub count: usize,
  pub previous: Option<String>,
  pub next: Option<String>,
  /// Its *Always allow* step is open.
  pub always: bool,
}

impl ApprovalQueue {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      selected: ctx.signal(None),
      deferred: ctx.signal(Vec::new()),
      always: ctx.signal(None),
      id: Arc::default(),
    }
  }

  /// Where the dialog stands among `pending`, oldest first; `None` while it is closed: nothing waits,
  /// or a request the person put off still does.
  pub fn place(&self, pending: &[ShownApproval]) -> Option<QueuePlace> {
    let deferred = self.deferred.get();
    if pending.iter().any(|shown| deferred.contains(&shown.approval.id)) {
      return None;
    }
    let selected = self.selected.get();
    let index = selected
      .and_then(|id| pending.iter().position(|shown| shown.approval.id == id))
      .unwrap_or(0);
    let shown = pending.get(index)?;
    let id = |at: Option<usize>| at.and_then(|at| pending.get(at)).map(|shown| shown.approval.id.clone());
    Some(QueuePlace {
      index,
      count: pending.len(),
      previous: id(index.checked_sub(1)),
      next: id(index.checked_add(1)),
      always: self.always.get().as_deref() == Some(shown.approval.id.as_str()),
    })
  }

  /// Opens the dialog on request `id`: *Review*, a row of *Needs you*, ‹ and ›.
  pub fn review(&self, id: &str) {
    self.always.set(None);
    self.deferred.set(Vec::new());
    self.selected.set(Some(id.to_owned()));
  }

  /// *Decide later*: closes the dialog over the requests `pending` now.
  pub fn defer(&self, pending: impl IntoIterator<Item = String>) {
    self.always.set(None);
    self.deferred.set(pending.into_iter().collect());
  }

  /// Opens or closes the *Always allow* step of request `id`.
  pub fn always(&self, id: Option<&str>) {
    self.always.set(id.map(str::to_owned));
  }
}
