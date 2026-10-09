//! Moving around the window: the tabs (`tabs`), each with its own history, and the router that shows
//! the active tab's place. Every navigation goes through [`Navigation`], which changes the tabs and then
//! has the router show the active tab's route; the router keeps no history of its own (it is always
//! replaced), so Back and Forward are the active tab's.

pub mod input;
pub mod shortcuts;
pub mod tabs;

use lurq::{app::ctx::Ctx, core::Signal, router::RouterHandle};

use self::tabs::{TabId, TabSet};
use crate::routes::Route;

/// The window's tabs and its router, shared by the shell, the shortcuts and the app's MCP tools. All of
/// them run on the event-loop thread.
#[derive(Clone)]
pub struct Navigation {
  tabs: Signal<TabSet>,
  router: RouterHandle,
}

impl Navigation {
  /// Tabs in `tabs`, shown by `router`, which goes to the active tab's route now.
  pub fn new(tabs: Signal<TabSet>, router: RouterHandle) -> Self {
    let navigation = Self { tabs, router };
    navigation.show();
    navigation
  }

  /// The navigation the root provides to everything it mounts.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the navigation before mounting the chrome and the shell")
  }

  /// The tabs; reading them in a render re-renders it when they change.
  pub fn tabs(&self) -> TabSet {
    self.tabs.get()
  }

  /// The tabs, without subscribing a render to them.
  pub fn tabs_now(&self) -> TabSet {
    self.tabs.get_untracked()
  }

  /// The active tab's route now.
  pub fn route_now(&self) -> Route {
    self.tabs.with_untracked(|tabs| tabs.active().route().clone())
  }

  /// Goes to `route` in the active tab, as a new history entry: another screen, or another tab of one
  /// (the Tasks list's or the Overview's workflow tab, a task's Summary, Conversation or Changes).
  pub fn go(&self, route: Route) {
    self.change(|tabs| tabs.go(route));
  }

  /// Shows `route` in place of the active tab's current place, without a history entry and keeping the
  /// places ahead: for what a screen refines while it stays on its tab, so Back leaves the screen rather
  /// than stepping through it (the Tasks list's search `q` as it is typed and its `highlight`, the file
  /// the Changes tab shows).
  pub fn replace(&self, route: Route) {
    self.change(|tabs| tabs.replace(route));
  }

  pub fn back(&self) {
    self.change(TabSet::back);
  }

  pub fn forward(&self) {
    self.change(TabSet::forward);
  }

  /// Opens a tab at All projects right of the active one and switches to it.
  pub fn open_tab(&self) {
    self.change(|tabs| {
      tabs.open();
      true
    });
  }

  pub fn select(&self, id: TabId) {
    self.change(|tabs| tabs.select(id));
  }

  /// Switches to the next tab (`step` 1) or the previous one (`step` -1), wrapping around.
  pub fn cycle(&self, step: isize) {
    self.change(|tabs| tabs.cycle(step));
  }

  pub fn close(&self, id: TabId) {
    self.change(|tabs| tabs.close(id));
  }

  /// Closes the active tab, when it can be closed.
  pub fn close_active(&self) {
    self.change(|tabs| tabs.close(tabs.active().id()));
  }

  /// Applies `change` to the tabs in place; when it changed them, shows the active route. A change
  /// that changed nothing still notifies the tabs' readers, which then render the same tabs.
  fn change(&self, change: impl FnOnce(&mut TabSet) -> bool) {
    let mut changed = false;
    self.tabs.update(|tabs| changed = change(tabs));
    if changed {
      self.show();
    }
  }

  fn show(&self) {
    let path = self.route_now().path();
    if self.router.path().get_untracked() != path {
      self.router.replace(path);
    }
  }
}
