//! The window's tabs (`docs/design/orchester-workspace.md`, "Tabs" › "Model"): each tab is an
//! independent place with its own Back/Forward history. A new tab opens All projects; closing the last
//! tab opens All projects in a fresh one, and the only tab cannot close while it shows All projects.

use std::fmt;

use crate::routes::Route;

/// The places a tab remembers; going somewhere past it forgets the oldest.
pub const HISTORY_LIMIT: usize = 50;

/// A tab's identity, stable while the tab is open and never reused in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(u64);

impl TabId {
  pub const fn get(self) -> u64 {
    self.0
  }
}

impl fmt::Display for TabId {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "{}", self.0)
  }
}

/// One tab: the places it has been, and where in them it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
  id: TabId,
  history: Vec<Route>,
  at: usize,
}

impl Tab {
  fn new(id: TabId, route: Route) -> Self {
    Self {
      id,
      history: vec![route],
      at: 0,
    }
  }

  pub const fn id(&self) -> TabId {
    self.id
  }

  pub fn route(&self) -> &Route {
    &self.history[self.at]
  }

  pub fn can_back(&self) -> bool {
    self.at > 0
  }

  pub fn can_forward(&self) -> bool {
    self.at + 1 < self.history.len()
  }

  /// Goes to `route`, dropping the places ahead and, past [`HISTORY_LIMIT`], the oldest; `false` when
  /// the tab is there already.
  fn go(&mut self, route: Route) -> bool {
    if *self.route() == route {
      return false;
    }
    self.history.truncate(self.at + 1);
    self.history.push(route);
    if self.history.len() > HISTORY_LIMIT {
      self.history.remove(0);
    }
    self.at = self.history.len() - 1;
    true
  }

  /// Puts `route` in place of the current entry, keeping the entries behind and ahead of it; `false`
  /// when the tab is there already.
  fn replace(&mut self, route: Route) -> bool {
    if *self.route() == route {
      return false;
    }
    self.history[self.at] = route;
    true
  }

  fn back(&mut self) -> bool {
    if !self.can_back() {
      return false;
    }
    self.at -= 1;
    true
  }

  fn forward(&mut self) -> bool {
    if !self.can_forward() {
      return false;
    }
    self.at += 1;
    true
  }
}

/// The open tabs and which one is active. There is always at least one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabSet {
  tabs: Vec<Tab>,
  active: usize,
  next_id: u64,
}

/// DevTools shows the tabs' signal by its type alone.
impl lurq::app::component::DevtoolsInspectable for TabSet {}

impl Default for TabSet {
  /// One tab, at All projects.
  fn default() -> Self {
    Self::at(Route::AllProjects)
  }
}

impl TabSet {
  /// One tab, at `route`: where a session opens.
  pub fn at(route: Route) -> Self {
    Self {
      tabs: vec![Tab::new(TabId(1), route)],
      active: 0,
      next_id: 2,
    }
  }

  pub fn tabs(&self) -> &[Tab] {
    &self.tabs
  }

  pub fn active(&self) -> &Tab {
    &self.tabs[self.active]
  }

  /// Goes to `route` in the active tab; `false` when it is there already.
  pub fn go(&mut self, route: Route) -> bool {
    self.tabs[self.active].go(route)
  }

  /// Puts `route` in place of the active tab's current place, without a history entry and keeping
  /// the places ahead; `false` when the tab is there already.
  pub fn replace(&mut self, route: Route) -> bool {
    self.tabs[self.active].replace(route)
  }

  pub fn back(&mut self) -> bool {
    self.tabs[self.active].back()
  }

  pub fn forward(&mut self) -> bool {
    self.tabs[self.active].forward()
  }

  /// Opens a tab at All projects right of the active one and activates it.
  pub fn open(&mut self) -> TabId {
    let id = self.new_id();
    self.active += 1;
    self.tabs.insert(self.active, Tab::new(id, Route::AllProjects));
    id
  }

  /// Activates the tab `id`; `false` when it is not open or active already.
  pub fn select(&mut self, id: TabId) -> bool {
    match self.position(id) {
      Some(index) if index != self.active => {
        self.active = index;
        true
      }
      _ => false,
    }
  }

  /// Activates the tab `step` places right of the active one (left when negative), wrapping around;
  /// `false` when there is only one tab.
  pub fn cycle(&mut self, step: isize) -> bool {
    let count = self.tabs.len();
    if count < 2 {
      return false;
    }
    let count = isize::try_from(count).expect("the window holds far fewer tabs than isize::MAX");
    let active = isize::try_from(self.active).expect("an index into the tabs fits in isize");
    self.active =
      usize::try_from((active + step).rem_euclid(count)).expect("rem_euclid of a positive count is positive");
    true
  }

  /// Whether the tab `id` can be closed: every tab but the only one while it shows All projects.
  pub fn can_close(&self, id: TabId) -> bool {
    self.position(id).is_some() && !(self.tabs.len() == 1 && *self.tabs[0].route() == Route::AllProjects)
  }

  /// Closes the tab `id`. Closing the active tab activates the next one to the right, or the left one
  /// when it was the last; closing the only tab replaces it with a fresh tab at All projects.
  pub fn close(&mut self, id: TabId) -> bool {
    if !self.can_close(id) {
      return false;
    }
    let Some(index) = self.position(id) else {
      return false;
    };
    if self.tabs.len() == 1 {
      let fresh = self.new_id();
      self.tabs[0] = Tab::new(fresh, Route::AllProjects);
      return true;
    }
    self.tabs.remove(index);
    if index < self.active || self.active == self.tabs.len() {
      self.active -= 1;
    }
    true
  }

  fn position(&self, id: TabId) -> Option<usize> {
    self.tabs.iter().position(|tab| tab.id == id)
  }

  fn new_id(&mut self) -> TabId {
    let id = TabId(self.next_id);
    self.next_id += 1;
    id
  }
}
