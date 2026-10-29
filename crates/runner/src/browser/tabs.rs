//! Which run each tab belongs to (`docs/design/browser.md`, decision 6): a tab a run opens is that run's, the run
//! reaches only its own, and they close when it ends. Each run has a current tab, which a call that names none acts
//! on: the one it opened or loaded a page in last, else the last it opened that is still open.
//!
//! A tab that is no longer a run's, closed or taken from an ended run, is no longer listened to: its log's follower
//! ([`super::log::follow`]) ends with it, so a browser that runs as long as the runner keeps nothing of closed tabs.

use std::{
  collections::HashMap,
  sync::{Arc, Weak},
};

use orchester_core::id::RunId;

use super::{
  cdp::{Cdp, SessionId, TargetId},
  tab::TabState,
};

/// The runs' open tabs, in the order they opened.
pub(super) struct RunTabs {
  cdp: Weak<Cdp>,
  tabs: Vec<RunTab>,
  current: HashMap<RunId, TargetId>,
  /// Held by each tab's log follower while it runs.
  followers: Arc<()>,
}

struct RunTab {
  target: TargetId,
  run: RunId,
  session: SessionId,
  state: Arc<TabState>,
  cdp: Weak<Cdp>,
}

impl Drop for RunTab {
  /// The tab's events are no longer listened to, so its log's follower ends.
  fn drop(&mut self) {
    if let Some(cdp) = self.cdp.upgrade() {
      cdp.unsubscribe(&self.session);
    }
  }
}

impl RunTabs {
  /// The tabs opened on the connection `cdp`.
  pub(super) fn new(cdp: &Arc<Cdp>) -> Self {
    Self {
      cdp: Arc::downgrade(cdp),
      tabs: Vec::new(),
      current: HashMap::new(),
      followers: Arc::new(()),
    }
  }

  /// Adds `run`'s new tab `target`, which becomes its current one.
  pub(super) fn add(&mut self, run: RunId, target: TargetId, session: SessionId, state: Arc<TabState>) {
    self.current.insert(run, target.clone());
    self.tabs.push(RunTab {
      target,
      run,
      session,
      state,
      cdp: Weak::clone(&self.cdp),
    });
  }

  /// What a tab's log follower holds while it runs, for [`RunTabs::followed`].
  pub(super) fn follower(&self) -> Arc<()> {
    Arc::clone(&self.followers)
  }

  /// How many tabs' logs are still followed.
  pub(super) fn followed(&self) -> usize {
    Arc::strong_count(&self.followers) - 1
  }

  /// The session and state of `target` when it is one of `run`'s tabs.
  pub(super) fn session(&self, run: RunId, target: &TargetId) -> Option<(SessionId, Arc<TabState>)> {
    self
      .tabs
      .iter()
      .find(|tab| tab.run == run && &tab.target == target)
      .map(|tab| (tab.session.clone(), Arc::clone(&tab.state)))
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
    self.tabs.retain(|tab| tab.run != run);
    self.current.remove(&run);
    taken
  }

  /// A tab the browser closed, whoever closed it.
  pub(super) fn forget(&mut self, target: &TargetId) {
    self.tabs.retain(|tab| &tab.target != target);
  }
}
