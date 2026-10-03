//! Which waiting request the approval dialog shows ("Approval choices and waiting requests"): one at a
//! time, the oldest first unless the person moved to another with ‹ › or opened one with *Review*.
//! *Decide later* closes the dialog and answers nothing, and puts off every request waiting then,
//! wherever it shows; while one of them still waits, the dialog does not open by itself, and a new
//! request only adds to the count. It opens by itself when nothing was put off. A request the person
//! opened (*Review*, its card's *Always allow…*) shows in the dialog also where it is otherwise inline.
//! A request the runner says no longer waits (its agent ended) leaves the dialog at once, and the
//! dialog says so over the next request until the person acts there (built, not drawn).

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
  /// The requests the runner said no longer wait, which the dialog no longer shows, even before the
  /// project's requests are read again.
  ended: Signal<Vec<String>>,
  /// A request ended while it was answered: the dialog says so until the person acts in it again.
  ended_notice: Signal<bool>,
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
      ended: ctx.signal(Vec::new()),
      ended_notice: ctx.signal(false),
      id: Arc::default(),
    }
  }

  /// The requests of `pending` the dialog may still show: those the runner has not said ended.
  pub fn still_waiting(&self, pending: &[ShownApproval]) -> Vec<ShownApproval> {
    let ended = self.ended.get();
    pending
      .iter()
      .filter(|shown| !ended.contains(&shown.approval.id))
      .cloned()
      .collect()
  }

  /// The runner answered that request `id` no longer waits: its agent can no longer receive an answer.
  /// The dialog drops it and says so over the next request.
  pub fn drop_ended(&self, id: &str) {
    if self.always.get_untracked().as_deref() == Some(id) {
      self.always.set(None);
    }
    self.ended.update(|ended| {
      if !ended.iter().any(|known| known == id) {
        ended.push(id.to_owned());
      }
    });
    self.ended_notice.set(true);
  }

  /// Whether the dialog says that a request it showed ended.
  pub fn shows_ended_notice(&self) -> bool {
    self.ended_notice.get()
  }

  /// The person acted in the dialog again: the notice that a request ended has been seen.
  pub fn clear_ended_notice(&self) {
    if self.ended_notice.get_untracked() {
      self.ended_notice.set(false);
    }
  }

  /// Where the dialog stands among `pending`, the requests it may show, oldest first; `None` while it
  /// is closed: none waits, or a request the person put off still waits among `all`, the project's.
  pub fn place(&self, pending: &[ShownApproval], all: &[ShownApproval]) -> Option<QueuePlace> {
    let deferred = self.deferred.get();
    if all.iter().any(|shown| deferred.contains(&shown.approval.id)) {
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
    self.clear_ended_notice();
    self.always.set(None);
    self.deferred.set(Vec::new());
    self.selected.set(Some(id.to_owned()));
  }

  /// *Decide later*: closes the dialog and puts off `pending`, every request waiting now.
  pub fn defer(&self, pending: impl IntoIterator<Item = String>) {
    self.clear_ended_notice();
    self.always.set(None);
    self.deferred.set(pending.into_iter().collect());
  }

  /// Whether the person opened request `id` (*Review*, ‹ ›, its card's *Always allow…*), which shows it
  /// in the dialog also where it is otherwise inline (its task's Conversation tab).
  pub fn opened(&self, id: &str) -> bool {
    self.selected.get().as_deref() == Some(id) || self.always.get().as_deref() == Some(id)
  }

  /// Opens the dialog on the *Always allow* step of request `id`, from its inline card.
  pub fn review_always(&self, id: &str) {
    self.review(id);
    self.always.set(Some(id.to_owned()));
  }

  /// Opens or closes the *Always allow* step of request `id`.
  pub fn always(&self, id: Option<&str>) {
    self.always.set(id.map(str::to_owned));
  }
}
