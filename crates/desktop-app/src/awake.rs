//! *Keep the computer awake while agents work* (`docs/plan/orchestrator-autonomy/OA-08-…`): while the person's
//! setting is on and work is under way in an open project, the app asks the system not to go to sleep by itself
//! (`orchester_exec::awake`), and lets go at once when the work ends or the setting is turned off. When work is
//! under way is the workspace's rule (`orchester_workspace::awake::work_under_way`): an agent works, or a turn of the
//! Orchestrator chat thinks. Each project's follower looks after every change of its runs and conversations
//! (`crate::project::waits`); nothing polls.
//!
//! Closing the window still stops the work: this keeps a computer that is left on from sleeping, nothing more.

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_exec::awake::Awake;
use parking_lot::Mutex;

/// The projects with work under way, and the hold on the system while there is one and the setting allows it.
#[derive(Clone, Default)]
pub struct KeepAwake {
  inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
  at_work: HashSet<PathBuf>,
  hold: Option<Awake>,
  /// The system refused the last hold: said once, not at every look.
  refused: bool,
}

impl KeepAwake {
  /// What a look at the project at `root` found: whether work is `under_way` there, and whether the person's setting
  /// is on (`allowed`). Holds the system awake while any project works and the setting is on; lets go otherwise.
  pub fn look(&self, root: &Path, under_way: bool, allowed: bool) {
    let mut inner = self.inner.lock();
    if under_way {
      inner.at_work.insert(root.to_path_buf());
    } else {
      inner.at_work.remove(root);
    }
    inner.settle(allowed);
  }

  /// The person turned the setting off: lets go at once, whatever works.
  pub fn release(&self) {
    self.inner.lock().settle(false);
  }

  /// Whether the system is held awake now.
  pub fn holds(&self) -> bool {
    self.inner.lock().hold.is_some()
  }
}

impl Inner {
  fn settle(&mut self, allowed: bool) {
    let wanted = allowed && !self.at_work.is_empty();
    if !wanted {
      self.hold = None;
      self.refused = false;
      return;
    }
    if self.hold.is_some() {
      return;
    }
    match Awake::hold() {
      Ok(hold) => {
        tracing::info!("keeping the computer awake while agents work");
        self.hold = Some(hold);
        self.refused = false;
      }
      Err(error) if !self.refused => {
        tracing::warn!(%error, "the computer is not kept awake while agents work");
        self.refused = true;
      }
      Err(_) => {}
    }
  }
}
