//! Whose each tab is (`docs/design/browser.md`, decision 6): a tab a run opens is that run's, the run reaches only its
//! own, and they close when it ends; a tab the person opens in the window is the person's, and no run reaches it. The
//! window reaches every tab. Each run has a current tab, which a call that names none acts on: the one it opened or
//! loaded a page in last, else the last it opened that is still open.
//!
//! A tab that is no longer kept, closed or taken from an ended run, is no longer listened to: its followers (the
//! window's, and a run's tab's log, [`super::log::follow`]) end with it, so a browser that runs as long as the runner
//! keeps nothing of closed tabs.

use std::{
  collections::HashMap,
  sync::{Arc, Weak},
};

use orchester_core::id::RunId;
use orchester_ipc::browser::TabOwner;
use tokio::task::JoinHandle;

use super::{
  cdp::{Cdp, SessionId, TargetId},
  tab::TabState,
};

/// The browser's open tabs, in the order they opened.
pub(super) struct Tabs {
  cdp: Weak<Cdp>,
  tabs: Vec<Tab>,
  current: HashMap<RunId, TargetId>,
  /// Held by each run's tab's log follower while it runs.
  followers: Arc<()>,
}

pub(super) struct Tab {
  pub(super) target: TargetId,
  pub(super) owner: TabOwner,
  pub(super) session: SessionId,
  state: Arc<TabState>,
  /// Its page is loading, as the tab's events last said.
  pub(super) loading: bool,
  /// Follows the tab's events for the window until the tab is no longer kept.
  window_follower: JoinHandle<()>,
  cdp: Weak<Cdp>,
}

impl Drop for Tab {
  /// The tab's events are no longer listened to, so its followers end.
  fn drop(&mut self) {
    self.window_follower.abort();
    if let Some(cdp) = self.cdp.upgrade() {
      cdp.unsubscribe(&self.session);
    }
  }
}

impl Tabs {
  /// The tabs opened on the connection `cdp`.
  pub(super) fn new(cdp: &Arc<Cdp>) -> Self {
    Self {
      cdp: Arc::downgrade(cdp),
      tabs: Vec::new(),
      current: HashMap::new(),
      followers: Arc::new(()),
    }
  }

  /// Adds `owner`'s new tab `target`; a run's becomes its current one.
  pub(super) fn add(
    &mut self,
    owner: TabOwner,
    (target, session): (TargetId, SessionId),
    state: Arc<TabState>,
    window_follower: JoinHandle<()>,
  ) {
    if let TabOwner::Run { run } = owner {
      self.current.insert(run, target.clone());
    }
    self.tabs.push(Tab {
      target,
      owner,
      session,
      state,
      loading: false,
      window_follower,
      cdp: Weak::clone(&self.cdp),
    });
  }

  /// What a run's tab's log follower holds while it runs, for [`Tabs::followed`].
  pub(super) fn follower(&self) -> Arc<()> {
    Arc::clone(&self.followers)
  }

  /// How many tabs' logs are still followed.
  pub(super) fn followed(&self) -> usize {
    Arc::strong_count(&self.followers) - 1
  }

  /// The session and state of `target` when it is one of `run`'s tabs; never of the person's or another run's.
  pub(super) fn session(&self, run: RunId, target: &TargetId) -> Option<(SessionId, Arc<TabState>)> {
    self
      .tabs
      .iter()
      .find(|tab| tab.owner == TabOwner::Run { run } && &tab.target == target)
      .map(|tab| (tab.session.clone(), Arc::clone(&tab.state)))
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

  /// `run`'s current tab; `None` while it has none open.
  pub(super) fn current(&self, run: RunId) -> Option<TargetId> {
    let open = self.of(run);
    self
      .current
      .get(&run)
      .filter(|current| open.contains(current))
      .cloned()
      .or_else(|| open.last().cloned())
  }

  /// Makes `target`, one of `run`'s tabs, its current one.
  pub(super) fn make_current(&mut self, run: RunId, target: &TargetId) {
    if self.session(run, target).is_some() {
      self.current.insert(run, target.clone());
    }
  }

  /// `run`'s tabs, no longer its: the run ended.
  pub(super) fn take(&mut self, run: RunId) -> Vec<TargetId> {
    let taken = self.of(run);
    self.tabs.retain(|tab| tab.owner != TabOwner::Run { run });
    self.current.remove(&run);
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
