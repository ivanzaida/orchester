//! Whose each tab is (`docs/design/browser.md`, decision 6): a tab a run opens is that run's, the run reaches only its
//! own, and they close when it ends; a tab the person opens in the window is the person's, and no run reaches it. The
//! window reaches every tab.

use orchester_core::id::RunId;
use orchester_ipc::browser::TabOwner;
use tokio::task::JoinHandle;

use super::cdp::{SessionId, TargetId};

/// The browser's open tabs, in the order they opened.
#[derive(Default)]
pub(super) struct Tabs {
  tabs: Vec<Tab>,
}

pub(super) struct Tab {
  pub(super) target: TargetId,
  pub(super) owner: TabOwner,
  pub(super) session: SessionId,
  /// Its page is loading, as the tab's events last said.
  pub(super) loading: bool,
  /// Follows the tab's events for the window until the tab is no longer kept.
  follower: JoinHandle<()>,
}

impl Drop for Tab {
  fn drop(&mut self) {
    self.follower.abort();
  }
}

impl Tabs {
  pub(super) fn add(&mut self, owner: TabOwner, target: TargetId, session: SessionId, follower: JoinHandle<()>) {
    self.tabs.push(Tab {
      target,
      owner,
      session,
      loading: false,
      follower,
    });
  }

  /// The session of `target` when it is one of `run`'s tabs; never of the person's or another run's.
  pub(super) fn session(&self, run: RunId, target: &TargetId) -> Option<SessionId> {
    self
      .tabs
      .iter()
      .find(|tab| tab.owner == TabOwner::Run { run } && &tab.target == target)
      .map(|tab| tab.session.clone())
  }

  /// `target` with its owner, whoever's it is: what the window reaches.
  pub(super) fn get(&self, target: &TargetId) -> Option<&Tab> {
    self.tabs.iter().find(|tab| &tab.target == target)
  }

  /// `run`'s tabs, in the order they opened.
  pub(super) fn of(&self, run: RunId) -> Vec<TargetId> {
    self
      .tabs
      .iter()
      .filter(|tab| tab.owner == TabOwner::Run { run })
      .map(|tab| tab.target.clone())
      .collect()
  }

  /// Every tab, in the order they opened.
  pub(super) fn all(&self) -> &[Tab] {
    &self.tabs
  }

  /// `run`'s tabs, no longer its: the run ended.
  pub(super) fn take(&mut self, run: RunId) -> Vec<TargetId> {
    let taken = self.of(run);
    self.tabs.retain(|tab| tab.owner != TabOwner::Run { run });
    taken
  }

  /// A tab the browser closed, whoever closed it; whether it was kept.
  pub(super) fn forget(&mut self, target: &TargetId) -> bool {
    let before = self.tabs.len();
    self.tabs.retain(|tab| &tab.target != target);
    self.tabs.len() != before
  }

  /// Whether `target`'s page loads now; whether that changed what was kept.
  pub(super) fn set_loading(&mut self, target: &TargetId, loading: bool) -> bool {
    match self.tabs.iter_mut().find(|tab| &tab.target == target) {
      Some(tab) if tab.loading != loading => {
        tab.loading = loading;
        true
      }
      _ => false,
    }
  }
}
