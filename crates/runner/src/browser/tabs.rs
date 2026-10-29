//! Which run each tab belongs to (`docs/design/browser.md`, decision 6): a tab a run opens is that run's, the run
//! reaches only its own, and they close when it ends.

use orchester_core::id::RunId;

use super::cdp::{SessionId, TargetId};

/// The runs' open tabs, in the order they opened.
#[derive(Default)]
pub(super) struct RunTabs {
  tabs: Vec<RunTab>,
}

struct RunTab {
  target: TargetId,
  run: RunId,
  session: SessionId,
}

impl RunTabs {
  pub(super) fn add(&mut self, run: RunId, target: TargetId, session: SessionId) {
    self.tabs.push(RunTab { target, run, session });
  }

  /// The session of `target` when it is one of `run`'s tabs.
  pub(super) fn session(&self, run: RunId, target: &TargetId) -> Option<SessionId> {
    self
      .tabs
      .iter()
      .find(|tab| tab.run == run && &tab.target == target)
      .map(|tab| tab.session.clone())
  }

  /// `run`'s tabs, in the order they opened.
  pub(super) fn of(&self, run: RunId) -> Vec<TargetId> {
    self
      .tabs
      .iter()
      .filter(|tab| tab.run == run)
      .map(|tab| tab.target.clone())
      .collect()
  }

  /// `run`'s tabs, no longer its: the run ended.
  pub(super) fn take(&mut self, run: RunId) -> Vec<TargetId> {
    let taken = self.of(run);
    self.tabs.retain(|tab| tab.run != run);
    taken
  }

  /// A tab the browser closed, whoever closed it.
  pub(super) fn forget(&mut self, target: &TargetId) {
    self.tabs.retain(|tab| &tab.target != target);
  }
}
