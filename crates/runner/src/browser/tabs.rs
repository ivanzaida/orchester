//! Whose each tab is (`docs/design/browser.md`, decision 6): a tab a run opens is that run's, the run reaches only its
//! own, and they close when it ends; a tab the person opens in the window is the person's, and no run reaches it. The
//! window reaches every tab. Each run has a current tab, which a call that names none acts on: the one it opened or
//! loaded a page in last, else the last it opened that is still open.
//!
//! A tab that is no longer kept, closed or taken from an ended run, is no longer listened to: its followers (the
//! window's, and a run's tab's log, [`super::log::follow`]) end with it, so a browser that runs as long as the runner
//! keeps nothing of closed tabs. Of a run's tab that someone other than the run closed, only its id and who closed it
//! are kept, until the run ends, so the run's next call on it says so ("Tabs").

use std::{
  collections::HashMap,
  sync::{Arc, Weak},
};

use orchester_core::id::RunId;
use orchester_ipc::browser::TabOwner;
use tokio::task::JoinHandle;

use super::{
  BrowserError,
  cdp::{Cdp, SessionId, TargetId},
  tab::TabState,
};

/// The browser's open tabs, in the order they opened.
pub(super) struct Tabs {
  cdp: Weak<Cdp>,
  tabs: Vec<Tab>,
  current: HashMap<RunId, TargetId>,
  /// Each run's tabs that someone other than the run closed, until the run ends.
  closed: HashMap<RunId, Vec<(TargetId, ClosedBy)>>,
  /// The tabs of the browsers before this one, by the id each has now: a run still names them so.
  moved: HashMap<TargetId, TargetId>,
  /// Held by each run's tab's log follower while it runs.
  followers: Arc<()>,
}

/// What a browser that starts again keeps of the tabs of the one before, besides the tabs themselves.
#[derive(Default)]
pub(super) struct Remembered {
  current: HashMap<RunId, TargetId>,
  closed: HashMap<RunId, Vec<(TargetId, ClosedBy)>>,
  moved: HashMap<TargetId, TargetId>,
}

/// Who closed a tab, other than the run whose it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClosedBy {
  /// The person, from the *Browser* window.
  Person,
  /// The browser reported it closed: its page closed itself, or the person closed it in the browser's own window.
  Browser,
}

pub(super) struct Tab {
  pub(super) target: TargetId,
  pub(super) owner: TabOwner,
  pub(super) session: SessionId,
  /// The title and address the browser last reported, all that is left of them once it ended.
  pub(super) title: String,
  pub(super) url: String,
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
      closed: HashMap::new(),
      moved: HashMap::new(),
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
      title: String::new(),
      url: "about:blank".to_owned(),
      state,
      loading: false,
      window_follower,
      cdp: Weak::clone(&self.cdp),
    });
  }

  /// What a browser that starts again in place of this one keeps of its tabs ([`Tabs::remember`]).
  pub(super) fn remembered(&self) -> Remembered {
    Remembered {
      current: self.current.clone(),
      closed: self.closed.clone(),
      moved: self.moved.clone(),
    }
  }

  /// Takes over what the browser before kept of its tabs, `reopened` here under new ids: a run's next call by an old id
  /// reaches the tab reopened from it, and its current tab stays the same page. An old id of a tab not reopened goes.
  pub(super) fn remember(&mut self, before: Remembered, reopened: Vec<(TargetId, TargetId)>) {
    let reopened: HashMap<TargetId, TargetId> = reopened.into_iter().collect();
    let now = |tab: &TargetId| reopened.get(tab).cloned();
    self.closed = before.closed;
    let current = before.current.iter();
    self.current = current.filter_map(|(run, tab)| Some((*run, now(tab)?))).collect();
    let moved = before.moved.iter();
    self.moved = moved.filter_map(|(old, tab)| Some((old.clone(), now(tab)?))).collect();
    self.moved.extend(reopened);
  }

  /// The id `target` has now: a tab of a browser before this one by the id it was reopened under, any other as it is.
  pub(super) fn now(&self, target: &TargetId) -> TargetId {
    self.moved.get(target).unwrap_or(target).clone()
  }

  /// What the browser reports of `target`'s page now; whether the tab is kept.
  pub(super) fn reported(&mut self, target: &TargetId, title: &str, url: &str) -> bool {
    let Some(tab) = self.tabs.iter_mut().find(|tab| &tab.target == target) else {
      return false;
    };
    title.clone_into(&mut tab.title);
    url.clone_into(&mut tab.url);
    true
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

  /// `run`'s tabs, no longer its: the run ended. What was kept of its tabs others closed goes too.
  pub(super) fn take(&mut self, run: RunId) -> Vec<TargetId> {
    let taken = self.of(run);
    self.tabs.retain(|tab| tab.owner != TabOwner::Run { run });
    self.current.remove(&run);
    self.closed.remove(&run);
    self.moved.retain(|_, tab| !taken.contains(tab));
    taken
  }

  /// A tab its run closed; whether it was kept.
  pub(super) fn forget(&mut self, target: &TargetId) -> bool {
    self.remove(target).is_some()
  }

  /// A tab `by` closed, whoever's it is; whether it was kept. A run's is remembered for the run until it ends.
  pub(super) fn closed_by(&mut self, target: &TargetId, by: ClosedBy) -> bool {
    let Some(owner) = self.remove(target) else {
      return false;
    };
    if let TabOwner::Run { run } = owner {
      self.closed.entry(run).or_default().push((target.clone(), by));
    }
    true
  }

  /// Why `target` is not one of `run`'s open tabs: someone other than the run closed it, or else it is not the run's.
  pub(super) fn refused(&self, run: RunId, target: &TargetId) -> BrowserError {
    let by = self
      .closed
      .get(&run)
      .and_then(|closed| closed.iter().find(|(tab, _)| tab == target))
      .map(|(_, by)| *by);
    match by {
      Some(ClosedBy::Person) => BrowserError::ClosedByPerson(target.clone()),
      Some(ClosedBy::Browser) => BrowserError::ClosedInBrowser(target.clone()),
      None => BrowserError::NotRunsTab(target.clone()),
    }
  }

  /// Removes `target`; its owner, when it was kept.
  fn remove(&mut self, target: &TargetId) -> Option<TabOwner> {
    let at = self.tabs.iter().position(|tab| &tab.target == target)?;
    Some(self.tabs.remove(at).owner)
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
