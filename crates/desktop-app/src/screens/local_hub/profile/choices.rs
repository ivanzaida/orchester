//! The choices made in a local profile's panel on their way to what the panel holds. A choice shows in its select at
//! once; what it makes of the profile (another CLI's first model, the access a CLI can keep, the readiness of what it
//! chooses) is the workspace's to say ([`local_edit::hold`]), off the UI thread, without locking anything the panel
//! shows. One batch is checked at a time: choices made meanwhile wait behind it and are made on what it held, so a
//! quick second choice never drops the first. *Cancel* drops what is checked and what waits; *Save* pressed meanwhile
//! saves once the choices are held.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::{id::ResourceId, resource::AgentProfile};
use orchester_workspace::{
  agents::local_edit::{self, Edit},
  background,
  messages::Message,
};
use parking_lot::Mutex;

use super::edit::Holding;

/// Choices checked together, made in order on `base` (what the panel held when they started; `None` for what is
/// stored), and the count of *Cancel* they were made under.
struct Batch {
  epoch: u64,
  base: Option<AgentProfile>,
  edits: Vec<Edit>,
}

/// What a batch made of the profile, under the count of *Cancel* it was made under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Chosen {
  epoch: u64,
  holding: Holding,
}
impl DevtoolsInspectable for Chosen {}

#[derive(Clone)]
pub(super) struct Choices {
  action: FutureAction<Batch, Chosen, Message>,
  held: Signal<Holding>,
  /// Choices made while a batch is checked, in order.
  waiting: Arc<Mutex<Vec<Edit>>>,
  /// Counts *Cancel*: a batch made before the last one is dropped when it ends.
  epoch: Arc<AtomicU64>,
  /// *Save* was pressed while choices were checked: it saves once they are held.
  save_after: Arc<AtomicBool>,
}

impl Choices {
  /// The choices of the profile `id` in the project at `root`, held in `held`; `save` saves what is held, for a *Save*
  /// pressed while they are checked.
  pub(super) fn new(
    ctx: &mut Ctx,
    (root, id): (PathBuf, ResourceId),
    held: &Signal<Holding>,
    save: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    let action = ctx.future_action(move |batch: Batch| {
      let root = root.clone();
      async move { background::run(move || made(&root, id, batch)).await? }
    });
    let choices = Self {
      action,
      held: held.clone(),
      waiting: Arc::default(),
      epoch: Arc::default(),
      save_after: Arc::default(),
    };
    let ended = choices.clone();
    ctx.watch(&choices.action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      let held = state.data.as_ref().filter(|_| state.is_fulfilled());
      if let Some(chosen) = held.filter(|chosen| chosen.epoch == ended.epoch.load(Ordering::SeqCst)) {
        ended.held.set(chosen.holding.clone());
      }
      if ended.next() {
        return;
      }
      if ended.save_after.swap(false, Ordering::SeqCst) && state.is_fulfilled() {
        save();
      }
    });
    choices
  }

  /// Makes `edit` on what the panel holds: now, or behind the batch that is checked. The role as typed replaces the
  /// one typed before it that still waits: it holds every field of the role, so only the last one is made.
  pub(super) fn make(&self, edit: Edit) {
    if self.action.state().get_untracked().is_pending() {
      let mut waiting = self.waiting.lock();
      if matches!((waiting.last(), &edit), (Some(Edit::Role(_)), Edit::Role(_))) {
        waiting.pop();
      }
      waiting.push(edit);
    } else {
      self.start(vec![edit]);
    }
  }

  /// Starts the choices that wait, on what the panel holds now; whether there were any.
  fn next(&self) -> bool {
    let waiting = std::mem::take(&mut *self.waiting.lock());
    if waiting.is_empty() {
      return false;
    }
    self.start(waiting);
    true
  }

  fn start(&self, edits: Vec<Edit>) {
    self.action.run(Batch {
      epoch: self.epoch.load(Ordering::SeqCst),
      base: self.held.get_untracked().body(),
      edits,
    });
  }

  /// Whether choices are being checked; read during render, so the panel follows it.
  pub(super) fn checking(&self) -> bool {
    self.action.state().get().is_pending()
  }

  /// Whether choices are being checked now, read in a handler.
  pub(super) fn waits(&self) -> bool {
    self.action.state().get_untracked().is_pending()
  }

  /// *Save* was pressed while choices are checked: it saves once they are held.
  pub(super) fn save_after(&self) {
    self.save_after.store(true, Ordering::SeqCst);
  }

  /// *Cancel*: what is checked and what waits are dropped, and a *Save* asked for meanwhile.
  pub(super) fn drop_all(&self) {
    self.epoch.fetch_add(1, Ordering::SeqCst);
    self.waiting.lock().clear();
    self.save_after.store(false, Ordering::SeqCst);
  }

  /// Why the last batch was refused, as the workspace said.
  pub(super) fn refusal(&self) -> Option<Message> {
    let state = self.action.state().get();
    state.error.clone().filter(|_| state.is_rejected())
  }
}

/// The choices of `batch` made in order on its base, as the panel holds them then.
fn made(root: &std::path::Path, id: ResourceId, batch: Batch) -> Result<Chosen, Message> {
  let mut body = batch.base;
  let mut holding = Holding::default();
  for edit in &batch.edits {
    let held = local_edit::hold(root, id, body.as_ref(), edit)?;
    body = Some(held.body.clone());
    holding = Holding(Some(held));
  }
  Ok(Chosen {
    epoch: batch.epoch,
    holding,
  })
}
