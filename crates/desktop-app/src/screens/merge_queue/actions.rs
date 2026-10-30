//! What the Merge queue's buttons do (`orchester_workspace::merge_queue`): a waiting run moves up or to the top, or
//! leaves the queue; *Retry* wakes the queue, which looks at the checkout again at once. Each change is made off the
//! UI thread; the queue's driver and the screen hear of it from the queue itself. A change that did not happen says
//! why on the screen.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::core::Signal;
use orchester_core::{id::RunId, time::Timestamp};
use orchester_workspace::{
  merge_queue::{self, Move, QueueRefusal},
  messages::Message,
};

use crate::ui::button::ActionButton;

/// The buttons' work for the queue of the project at `root`; why the last change did not happen goes to `failed`.
#[derive(Clone)]
pub(super) struct QueueActions {
  root: Arc<PathBuf>,
  failed: Signal<Option<Message>>,
}

impl QueueActions {
  pub(super) fn new(root: &Path, failed: Signal<Option<Message>>) -> Self {
    Self {
      root: Arc::new(root.to_path_buf()),
      failed,
    }
  }

  /// Moves `run` as `how` says.
  pub(super) fn move_run(&self, run: RunId, how: Move) {
    self.change(move |root| merge_queue::move_run(root, run, how));
  }

  /// Takes `run` out of the queue.
  pub(super) fn remove(&self, run: RunId) {
    self.change(move |root| merge_queue::remove(root, run, Timestamp::now()));
  }

  /// `button` making the queue look again now, as a blocked delivery's *Retry*.
  pub(super) fn retries(&self, button: ActionButton) -> ActionButton {
    let root = self.root.clone();
    button.on_click(move || merge_queue::wake(&root))
  }

  /// Makes `change` on a thread of its own.
  fn change(&self, change: impl FnOnce(&Path) -> Result<(), QueueRefusal> + Send + 'static) {
    let (root, failed) = (self.root.clone(), self.failed.clone());
    failed.set(None);
    let started = std::thread::Builder::new()
      .name("merge-queue-change".to_owned())
      .spawn(move || {
        if let Err(refusal) = change(&root) {
          tracing::info!(code = refusal.code(), "a change of the merge queue did not happen");
          failed.set(Some(refusal.message()));
        }
      });
    if let Err(error) = started {
      tracing::error!(%error, "cannot start changing the merge queue");
      self.failed.set(Some(orchester_workspace::messages::interrupted()));
    }
  }
}
