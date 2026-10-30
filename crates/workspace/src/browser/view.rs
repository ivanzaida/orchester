//! What the *Browser* window knows of one project's browser, from its runner's events: its tabs with their owners,
//! the tab shown and the one shown last, the latest frame, what an agent did last on each tab (*Agent is acting*),
//! which tabs an agent waits on because the person uses them and until when, the JavaScript dialogs waiting on the
//! person's tabs and the `<select>` list the person opened.

use std::{collections::HashMap, sync::Arc, time::Instant};

use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::browser::{
  AgentAction, BrowserDialog, BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, SelectList,
};

use super::RgbaFrame;

/// What an agent did last on a tab, and when the window learnt it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentActing {
  pub run: RunId,
  pub action: AgentAction,
  pub at: Instant,
}

/// An agent of `run` waits on a tab because the person uses it, until `until` as the runner last said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentWaiting {
  pub run: RunId,
  pub until: Timestamp,
}

/// A `<select>` list the person opened on a tab, which the window draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSelect {
  pub tab: BrowserTabId,
  pub list: SelectList,
}

/// One project's browser as the window shows it.
#[derive(Clone, Debug)]
pub struct BrowserView {
  name: Arc<str>,
  state: BrowserState,
  last_shown: Option<BrowserTabId>,
  frame: Option<Arc<RgbaFrame>>,
  acting: HashMap<BrowserTabId, AgentActing>,
  waiting: HashMap<BrowserTabId, AgentWaiting>,
  dialogs: HashMap<BrowserTabId, BrowserDialog>,
  select: Option<OpenSelect>,
}

impl BrowserView {
  pub(super) fn new(name: Arc<str>) -> Self {
    Self {
      name,
      state: BrowserState::default(),
      last_shown: None,
      frame: None,
      acting: HashMap::new(),
      waiting: HashMap::new(),
      dialogs: HashMap::new(),
      select: None,
    }
  }

  /// The project's name, as the project switcher lists it.
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Its tabs, in the order they opened; none while the browser does not run.
  pub fn tabs(&self) -> &[BrowserTab] {
    &self.state.tabs
  }

  /// The tab shown, if the window shows one: screencast now, or only named with the browser in a window of its own.
  pub fn shown(&self) -> Option<&BrowserTabId> {
    self.state.shown.as_ref()
  }

  /// The tab shown last, still open: the one to show again when the window comes back to the project.
  pub fn last_shown(&self) -> Option<&BrowserTabId> {
    self.last_shown.as_ref()
  }

  pub fn mode(&self) -> BrowserMode {
    self.state.mode
  }

  /// The latest frame of the tab shown; none before its first frame, and none while no tab is shown.
  pub fn frame(&self) -> Option<&Arc<RgbaFrame>> {
    self
      .frame
      .as_ref()
      .filter(|frame| self.state.shown.as_ref() == Some(frame.tab()))
  }

  /// What an agent did last on `tab`.
  pub fn acting(&self, tab: &BrowserTabId) -> Option<&AgentActing> {
    self.acting.get(tab)
  }

  /// The latest action of an agent on any of the project's tabs, for the project switcher.
  pub fn latest_acting(&self) -> Option<&AgentActing> {
    self.acting.values().max_by_key(|acting| acting.at)
  }

  /// The agent waiting on `tab` because the person uses it.
  pub fn waiting(&self, tab: &BrowserTabId) -> Option<&AgentWaiting> {
    self.waiting.get(tab)
  }

  /// The JavaScript dialog waiting on the person's `tab`.
  pub fn dialog(&self, tab: &BrowserTabId) -> Option<&BrowserDialog> {
    self.dialogs.get(tab)
  }

  pub fn select(&self) -> Option<&OpenSelect> {
    self.select.as_ref()
  }

  /// Applies the runner's `event`. An agent starting to browse changes nothing here (see `BrowserWindow`).
  pub(super) fn apply(&mut self, event: BrowserEvent) {
    match event {
      BrowserEvent::Changed(state) => self.changed(state),
      BrowserEvent::AgentStartedBrowsing { .. } => {}
      BrowserEvent::AgentActed { run, tab, action } => {
        let at = Instant::now();
        self.acting.insert(tab, AgentActing { run, action, at });
      }
      BrowserEvent::AgentWaiting { run, tab, until } => {
        self.waiting.insert(tab, AgentWaiting { run, until });
      }
      BrowserEvent::AgentWaitEnded { tab, .. } => {
        self.waiting.remove(&tab);
      }
      BrowserEvent::DialogOpened { tab, dialog } => {
        self.dialogs.insert(tab, dialog);
      }
      BrowserEvent::DialogClosed { tab } => {
        self.dialogs.remove(&tab);
      }
      BrowserEvent::SelectOpened { tab, list } => self.select = Some(OpenSelect { tab, list }),
    }
  }

  /// The browser's state now: what belonged to a tab that closed goes with it.
  fn changed(&mut self, state: BrowserState) {
    let open = |tab: &BrowserTabId| state.tabs.iter().any(|open| &open.id == tab);
    self.acting.retain(|tab, _| open(tab));
    self.waiting.retain(|tab, _| open(tab));
    self.dialogs.retain(|tab, _| open(tab));
    if self.select.as_ref().is_some_and(|select| !open(&select.tab)) {
      self.select = None;
    }
    if self.frame.as_ref().is_some_and(|frame| !open(frame.tab())) {
      self.frame = None;
    }
    if let Some(shown) = &state.shown {
      self.last_shown = Some(shown.clone());
    } else if self.last_shown.as_ref().is_some_and(|tab| !open(tab)) {
      self.last_shown = None;
    }
    self.state = state;
  }

  /// `frame` was decoded: it replaces an earlier frame, never a later one. A frame may come before the state that
  /// lists its tab as shown; [`Self::frame`] gives it only once its tab is the one shown.
  pub(super) fn decoded(&mut self, frame: RgbaFrame) {
    if self
      .frame
      .as_ref()
      .is_some_and(|kept| kept.sequence() > frame.sequence())
    {
      return;
    }
    self.frame = Some(Arc::new(frame));
  }

  pub(super) fn close_select(&mut self) -> bool {
    self.select.take().is_some()
  }
}
