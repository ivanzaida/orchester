//! The window's tabs (`docs/design/orchester-workspace.md`, "Tabs" › "Model"): each tab is an
//! independent place with its own Back/Forward history. A new tab opens All projects; closing the last
//! tab opens All projects in a fresh one, and the only tab cannot close while it shows All projects.
//!
//! An item of a project a person opens (what waits in Needs you, a notification's place, a chat's link) keeps each
//! project in its own tab ([`TabSet::open_item`]): another project's item never takes the active tab's place. The
//! tab it opens in remembers the tab it came from at the entry it arrived at, and Back on that entry returns to that
//! tab, once, as Back within a tab returns to the place before; Forward stays within the tab. The link goes when
//! Back has used it, when the entry leaves the tab's history, or when the tab it came from closed.

use std::fmt;

use orchester_core::id::ProjectId;

use crate::routes::{Route, Screen};

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

/// DevTools shows a signal holding a tab's id by its type alone.
impl lurq::app::component::DevtoolsInspectable for TabId {}

impl fmt::Display for TabId {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "{}", self.0)
  }
}

/// How a tab was reached from another one by an item of its project: the entry it arrived at, and the tab Back on
/// that entry returns to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Arrival {
  at: usize,
  from: TabId,
}

/// One tab: the places it has been, where in them it is, and the project it was in last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
  id: TabId,
  history: Vec<Route>,
  at: usize,
  /// The project of the last place the tab showed in one, kept while it shows places outside projects.
  last_project: Option<ProjectId>,
  arrival: Option<Arrival>,
}

impl Tab {
  fn new(id: TabId, route: Route) -> Self {
    Self {
      id,
      last_project: route.project(),
      history: vec![route],
      at: 0,
      arrival: None,
    }
  }

  pub const fn id(&self) -> TabId {
    self.id
  }

  pub fn route(&self) -> &Route {
    &self.history[self.at]
  }

  /// The place Back returns to, if any: where a page was opened from, for its *Cancel* to go back to.
  pub fn behind(&self) -> Option<&Route> {
    self.at.checked_sub(1).map(|at| &self.history[at])
  }

  /// The project the sidebar shows: the route's, and on the app's Settings, which belong to no project,
  /// the one the tab was in last; `None` when the tab has not been in a project.
  pub fn sidebar_project(&self) -> Option<ProjectId> {
    let route = self.route();
    match route.project() {
      Some(project) => Some(project),
      None if route.screen() == Screen::Settings => self.last_project,
      None => None,
    }
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
    if self.arrival.is_some_and(|arrival| arrival.at > self.at) {
      self.arrival = None;
    }
    self.history.push(route);
    if self.history.len() > HISTORY_LIMIT {
      self.remove(0);
    }
    self.at = self.history.len() - 1;
    self.arrived()
  }

  /// Puts `route` in place of the current entry, keeping the entries behind and ahead of it; `false`
  /// when the tab is there already.
  fn replace(&mut self, route: Route) -> bool {
    if *self.route() == route {
      return false;
    }
    self.history[self.at] = route;
    self.arrived()
  }

  /// Leaves the current entry for `route`: drops the entry when the one behind it is `route`, so neither
  /// Back nor Forward comes to it again, and puts `route` in its place otherwise; `false` when the tab is
  /// there already.
  fn close_panel(&mut self, route: Route) -> bool {
    if *self.route() == route {
      return false;
    }
    if self.at > 0 && self.history[self.at - 1] == route {
      self.remove(self.at);
      self.at -= 1;
      return self.arrived();
    }
    self.replace(route)
  }

  /// Drops the entry at `index`, and the tab it was reached from with it when it was the entry it arrived at.
  fn remove(&mut self, index: usize) {
    self.history.remove(index);
    self.arrival = match self.arrival {
      Some(arrival) if arrival.at == index => None,
      Some(Arrival { at, from }) if at > index => Some(Arrival { at: at - 1, from }),
      kept => kept,
    };
  }

  /// Remembers that the tab arrived at its current entry from the tab `from`.
  fn arrive_from(&mut self, from: TabId) {
    self.arrival = Some(Arrival { at: self.at, from });
  }

  /// The tab Back returns to from the current entry, when the tab arrived at it from another one.
  fn came_from(&self) -> Option<TabId> {
    self
      .arrival
      .filter(|arrival| arrival.at == self.at)
      .map(|arrival| arrival.from)
  }

  fn back(&mut self) -> bool {
    if !self.can_back() {
      return false;
    }
    self.at -= 1;
    self.arrived()
  }

  fn forward(&mut self) -> bool {
    if !self.can_forward() {
      return false;
    }
    self.at += 1;
    self.arrived()
  }

  /// Remembers the project of the place the tab shows now, if it is in one; `true`, as the tab moved.
  fn arrived(&mut self) -> bool {
    if let Some(project) = self.route().project() {
      self.last_project = Some(project);
    }
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

  /// Leaves the active tab's current place, a panel, for `route`: back to it when the tab came from there,
  /// dropping the panel's entry, and in its place otherwise; `false` when the tab is there already.
  pub fn close_panel(&mut self, route: Route) -> bool {
    self.tabs[self.active].close_panel(route)
  }

  /// Goes back in the active tab, or, on the entry it arrived at from another tab that is still open, to that tab
  /// ([`open_item`](Self::open_item)), which shows what it showed; `false` when there is nowhere to go back to.
  pub fn back(&mut self) -> bool {
    if let Some(index) = self.came_from() {
      self.tabs[self.active].arrival = None;
      self.active = index;
      return true;
    }
    self.tabs[self.active].back()
  }

  /// Whether [`back`](Self::back) goes somewhere.
  pub fn can_back(&self) -> bool {
    self.came_from().is_some() || self.active().can_back()
  }

  pub fn forward(&mut self) -> bool {
    self.tabs[self.active].forward()
  }

  /// Goes to `route`, an item a person opens, keeping each project in its own tab: in the active tab when it shows
  /// the item's project (on the app's Settings, the one it was in last); else in a tab of that project, the one at
  /// the item first, which becomes active; else in the active tab when it shows no project, and in a new tab right of
  /// it otherwise. A tab it switches to remembers the tab it came from for Back. `false` when the active tab is
  /// there already.
  pub fn open_item(&mut self, route: Route) -> bool {
    let Some(project) = route.project() else {
      return self.go(route);
    };
    let here = self.active().sidebar_project();
    if here == Some(project) {
      return self.go(route);
    }
    let from = self.active().id();
    let tab = self
      .tabs
      .iter()
      .position(|tab| *tab.route() == route)
      .or_else(|| self.tabs.iter().position(|tab| tab.sidebar_project() == Some(project)));
    match tab {
      Some(index) => {
        self.active = index;
        self.tabs[index].go(route);
      }
      None if here.is_none() => return self.go(route),
      None => {
        self.open_at(route, true);
      }
    }
    self.tabs[self.active].arrive_from(from);
    true
  }

  /// The index of the open tab Back on the active tab's entry returns to, if any.
  fn came_from(&self) -> Option<usize> {
    self.active().came_from().and_then(|from| self.position(from))
  }

  /// Opens a tab at All projects right of the active one and activates it.
  pub fn open(&mut self) -> TabId {
    self.open_at(Route::AllProjects, true)
  }

  /// Opens a tab at `route` right of the active one; it becomes the active one when `switch`, and otherwise
  /// waits in the background while the active tab stays as it is.
  pub fn open_at(&mut self, route: Route, switch: bool) -> TabId {
    let id = self.new_id();
    let index = self.active + 1;
    self.tabs.insert(index, Tab::new(id, route));
    if switch {
      self.active = index;
    }
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

  /// Whether the tab `id` can be closed: every tab but the only one while it shows All projects. The only tab
  /// on another place, Add project too, closes into a fresh tab at All projects.
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
