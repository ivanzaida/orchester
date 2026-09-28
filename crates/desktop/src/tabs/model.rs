//! The window's tabs as data (design doc, "Tabs"): each tab is a location — a router path — and
//! one of them is active. New tabs open right of the active one. Closing the last tab leaves a
//! fresh tab on the home page (All projects) instead of an empty window, and a sole home tab
//! cannot be closed. What the window saves between sessions is the paths and the active tab,
//! never the history ([`SavedTabs`]).

use std::sync::Arc;

/// Identifies a tab for as long as it is open; a closed tab's id is never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(u64);

impl TabId {
  pub const fn get(self) -> u64 {
    self.0
  }
}

impl std::fmt::Display for TabId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
  id: TabId,
  path: Arc<str>,
}

impl Tab {
  pub const fn id(&self) -> TabId {
    self.id
  }

  /// Where the tab is: the path its router shows.
  pub fn path(&self) -> &Arc<str> {
    &self.path
  }
}

/// Whether a tab opened in the background or became the active one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
  Switch,
  Background,
}

/// What closing a tab did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closed {
  /// It was closed; another tab is active if it was the active one.
  Closed,
  /// It was the last tab: a fresh home tab took its place.
  Replaced,
  /// Nothing: the tab is not open, or it is the sole home tab.
  Kept,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabSet {
  tabs: Vec<Tab>,
  active: TabId,
  /// The home page's path, which a new tab opens.
  home: Arc<str>,
  next: u64,
}

impl TabSet {
  /// One tab at `path`, active; new tabs open `home`.
  pub fn new(home: &str, path: &str) -> Self {
    let first = Tab {
      id: TabId(1),
      path: path.into(),
    };
    Self {
      active: first.id,
      tabs: vec![first],
      home: home.into(),
      next: 2,
    }
  }

  /// The tabs of `saved`, in their order; `None` when it holds none.
  pub fn restored(home: &str, saved: &SavedTabs) -> Option<Self> {
    let (first, rest) = saved.paths.split_first()?;
    let mut set = Self::new(home, first);
    for path in rest {
      set.push(path);
    }
    let active = set.tabs.get(saved.active).map_or(set.active, Tab::id);
    set.active = active;
    Some(set)
  }

  pub fn tabs(&self) -> &[Tab] {
    &self.tabs
  }

  pub fn len(&self) -> usize {
    self.tabs.len()
  }

  pub fn is_empty(&self) -> bool {
    self.tabs.is_empty()
  }

  pub const fn active(&self) -> TabId {
    self.active
  }

  pub fn active_index(&self) -> usize {
    self.index(self.active).expect("the active tab is open")
  }

  pub fn active_tab(&self) -> &Tab {
    &self.tabs[self.active_index()]
  }

  pub fn home(&self) -> &Arc<str> {
    &self.home
  }

  pub fn index(&self, id: TabId) -> Option<usize> {
    self.tabs.iter().position(|tab| tab.id == id)
  }

  pub fn tab(&self, id: TabId) -> Option<&Tab> {
    self.tabs.iter().find(|tab| tab.id == id)
  }

  /// Opens `path` in a new tab right of the active one.
  pub fn open(&mut self, path: &str, focus: Focus) -> TabId {
    let tab = self.new_tab(path);
    let id = tab.id;
    self.tabs.insert(self.active_index() + 1, tab);
    if focus == Focus::Switch {
      self.active = id;
    }
    id
  }

  /// A new tab on the home page, active.
  pub fn open_home(&mut self) -> TabId {
    let home = self.home.clone();
    self.open(&home, Focus::Switch)
  }

  /// Makes `id` the active tab; `false` when it is not open or already active.
  pub fn activate(&mut self, id: TabId) -> bool {
    if self.active == id || self.index(id).is_none() {
      return false;
    }
    self.active = id;
    true
  }

  /// Whether `id` can be closed: every tab but a sole tab on the home page.
  pub fn closable(&self, id: TabId) -> bool {
    match self.tab(id) {
      Some(tab) => self.tabs.len() > 1 || tab.path != self.home,
      None => false,
    }
  }

  /// Closes `id`. The tab right of it becomes active, or the one left of it when it was the last;
  /// closing an inactive tab keeps the active one.
  pub fn close(&mut self, id: TabId) -> Closed {
    if !self.closable(id) {
      return Closed::Kept;
    }
    let index = self.index(id).expect("a closable tab is open");
    if self.tabs.len() == 1 {
      let fresh = self.new_tab(&self.home.clone());
      self.active = fresh.id;
      self.tabs = vec![fresh];
      return Closed::Replaced;
    }
    self.tabs.remove(index);
    if self.active == id {
      let next = index.min(self.tabs.len() - 1);
      self.active = self.tabs[next].id;
    }
    Closed::Closed
  }

  /// Activates the tab right of the active one, or the first after the last.
  pub fn activate_next(&mut self) -> bool {
    let next = (self.active_index() + 1) % self.tabs.len();
    self.activate(self.tabs[next].id)
  }

  /// Activates the tab left of the active one, or the last before the first.
  pub fn activate_previous(&mut self) -> bool {
    let len = self.tabs.len();
    let previous = (self.active_index() + len - 1) % len;
    self.activate(self.tabs[previous].id)
  }

  /// Activates the tab at `index` (from 0), if there is one.
  pub fn select(&mut self, index: usize) -> bool {
    match self.tabs.get(index) {
      Some(tab) => {
        let id = tab.id;
        self.activate(id)
      }
      None => false,
    }
  }

  pub fn select_last(&mut self) -> bool {
    self.select(self.tabs.len() - 1)
  }

  /// Moves `id` to `index` among the tabs (clamped to the end); the active tab stays active.
  pub fn move_tab(&mut self, id: TabId, index: usize) -> bool {
    let Some(from) = self.index(id) else {
      return false;
    };
    let to = index.min(self.tabs.len() - 1);
    if from == to {
      return false;
    }
    let tab = self.tabs.remove(from);
    self.tabs.insert(to, tab);
    true
  }

  /// Records that `id` now shows `path`; `false` when it already did or is not open.
  pub fn set_path(&mut self, id: TabId, path: &str) -> bool {
    match self.tabs.iter_mut().find(|tab| tab.id == id) {
      Some(tab) if *tab.path != *path => {
        tab.path = path.into();
        true
      }
      _ => false,
    }
  }

  /// What the window saves of its tabs.
  pub fn saved(&self) -> SavedTabs {
    SavedTabs {
      paths: self.tabs.iter().map(|tab| tab.path.to_string()).collect(),
      active: self.active_index(),
    }
  }

  fn push(&mut self, path: &str) {
    let tab = self.new_tab(path);
    self.tabs.push(tab);
  }

  fn new_tab(&mut self, path: &str) -> Tab {
    let id = TabId(self.next);
    self.next += 1;
    Tab { id, path: path.into() }
  }
}

impl lurq::app::component::DevtoolsInspectable for TabSet {}

/// The tabs as the application database keeps them (`ui_state`): their paths in order and the
/// active one, without their Back/Forward history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTabs {
  pub paths: Vec<String>,
  pub active: usize,
}

/// The first line of the saved text, naming its format.
const FORMAT: &str = "orchester-tabs 1";
const ACTIVE: &str = "active ";

impl SavedTabs {
  /// One line naming the format, one with the active tab, then one path per line. Paths never
  /// hold a line break: the router's paths are URL paths.
  pub fn encode(&self) -> String {
    let mut text = format!("{FORMAT}\n{ACTIVE}{}\n", self.active);
    for path in &self.paths {
      text.push_str(path);
      text.push('\n');
    }
    text
  }

  /// The tabs [`SavedTabs::encode`] wrote; `None` for any other text, which is then ignored. An
  /// active tab that is not there is the first.
  pub fn decode(text: &str) -> Option<Self> {
    let mut lines = text.lines();
    if lines.next()? != FORMAT {
      return None;
    }
    let active = lines.next()?.strip_prefix(ACTIVE)?.parse::<usize>().ok()?;
    let paths: Vec<String> = lines.map(str::to_owned).collect();
    if paths.is_empty() || paths.iter().any(|path| !path.starts_with('/')) {
      return None;
    }
    let active = if active < paths.len() { active } else { 0 };
    Some(Self { paths, active })
  }
}
