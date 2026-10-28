//! Moving around the window: the tabs (`tabs`), each with its own history, and the router that shows
//! the active tab's place. Every navigation goes through [`Navigation`], which changes the tabs and then
//! has the router show the active tab's route; the router keeps no history of its own (it is always
//! replaced), so Back and Forward are the active tab's, except that Back on the place an item of another project
//! opened in returns to the tab it was opened from (`tabs`, [`Navigation::open_item`]). Each Orchester Cloud account
//! has tabs of its own (`accounts`).

pub mod accounts;
pub mod guard;
pub mod input;
pub mod link;
pub mod pointed;
pub mod press;
pub mod shortcuts;
pub mod tabs;
pub mod waiting;

use std::sync::{
  Arc,
  atomic::{AtomicU64, Ordering},
};

use lurq::{
  app::ctx::Ctx,
  core::{ElementRef, Signal},
  router::RouterHandle,
};

use self::{
  pointed::Pointed,
  press::OpenIn,
  tabs::{TabId, TabSet},
};
use crate::routes::Route;

/// The window's tabs and its router, shared by the shell, the shortcuts and the app's MCP tools. All of
/// them run on the event-loop thread.
#[derive(Clone)]
pub struct Navigation {
  tabs: Signal<TabSet>,
  router: RouterHandle,
  guard: std::sync::Arc<parking_lot::Mutex<std::sync::Weak<guard::PendingLeave>>>,
  /// The card the last navigation points at, numbered among every pointing, until the next navigation.
  pointed: Signal<Option<(u64, Pointed)>>,
  /// How many navigations pointed at a card; the last one's number.
  pointings: Arc<AtomicU64>,
  /// The number of the last pointing a card took: a card takes each pointing once.
  taken: Arc<AtomicU64>,
  /// Whether *Jump to* (`shell::jump`) is open over the window.
  jump: Signal<bool>,
  /// The content panel, which *Jump to* opens over.
  jump_anchor: ElementRef,
}

impl Navigation {
  /// Tabs in `tabs`, shown by `router`, which goes to the active tab's route now.
  pub fn new(tabs: Signal<TabSet>, router: RouterHandle) -> Self {
    let navigation = Self {
      tabs,
      router,
      guard: Default::default(),
      pointed: Signal::new(None),
      pointings: Arc::default(),
      taken: Arc::default(),
      jump: Signal::new(false),
      jump_anchor: ElementRef::new(),
    };
    navigation.show();
    navigation
  }

  /// The navigation the root provides to everything it mounts.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the navigation before mounting the chrome and the shell")
  }

  /// Whether *Jump to* is open; the sidebar's row and Ctrl+K open it, a choice, Esc or a press outside close it.
  pub fn jump(&self) -> Signal<bool> {
    self.jump.clone()
  }

  /// The element *Jump to* opens over: the shell's content panel.
  pub fn jump_anchor(&self) -> ElementRef {
    self.jump_anchor.clone()
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

  /// Goes to `route` in the active tab, as a new history entry, as a browser does: for every place a
  /// person chooses — another screen, another tab of one (`PageTab`), a filter, an item of a list or a
  /// panel — so Back returns to the one before and Forward comes back.
  pub fn go(&self, route: Route) {
    self.change(|tabs| tabs.go(route));
  }

  /// Shows `route` in place of the active tab's current place, without a history entry and keeping the
  /// places ahead: only where no new place is chosen — a route the app corrects (a removed item, a saved
  /// draft that got its id, a new tab's first place) and a search refined as it is typed, which would
  /// otherwise add an entry per keystroke.
  pub fn replace(&self, route: Route) {
    self.change(|tabs| tabs.replace(route));
  }

  /// Closes a panel (*Cancel*, *Finish later*) for `route` without leaving a dead entry: back to `route`
  /// when the panel was opened from it, dropping the panel's entry, and in the panel's place otherwise.
  pub fn close_panel(&self, route: Route) {
    self.change(|tabs| tabs.close_panel(route));
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

  /// Opens `route` where `open` says: in the active tab as [`go`](Self::go) does, or in a new tab right of the
  /// active one, in the background or switched to.
  pub fn open(&self, route: Route, open: OpenIn) {
    match open {
      OpenIn::ActiveTab => self.go(route),
      OpenIn::BackgroundTab | OpenIn::NewTab => self.change(|tabs| {
        tabs.open_at(route, open == OpenIn::NewTab);
        true
      }),
    }
  }

  /// Opens `route`, an item of a project a person opens (what waits in Needs you, a notification's place, a chat's
  /// link, a status bar event), where `open` says: a plain press keeps each project in its own tab
  /// ([`TabSet::open_item`]), so another project's item never takes the active tab's place; a press for a new tab
  /// opens one as [`open`](Self::open) does.
  pub fn open_item(&self, route: Route, open: OpenIn) {
    match open {
      OpenIn::ActiveTab => self.change(|tabs| tabs.open_item(route)),
      OpenIn::BackgroundTab | OpenIn::NewTab => self.open(route, open),
    }
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

  /// Shows `tabs` in place of the window's tabs, whatever those hold, and returns them: the tabs of another account
  /// signing in take the place of the last one's (`root`), with no leave guard, which held that account's edits.
  pub fn swap_tabs(&self, tabs: TabSet) -> TabSet {
    let before = self.tabs_now();
    self.apply(tabs);
    before
  }

  /// Opens `route`, an item, like [`open_item`](Self::open_item), pointing at `card` on the page it opens when the
  /// window shows it then, also when the active tab is there already: a page opened in a background tab is not
  /// shown, and nothing points at its card. A departure an editor holds points at nothing.
  pub fn open_to_card(&self, route: &Route, open: OpenIn, card: Pointed) {
    self.open_item(route.clone(), open);
    if open != OpenIn::BackgroundTab && self.route_now() == *route {
      let number = self.pointings.fetch_add(1, Ordering::Relaxed) + 1;
      self.pointed.set(Some((number, card)));
    }
  }

  /// Takes the pointer when it points at `card`: `true` once, for the card's first render after the
  /// navigation. Reading it in a render re-renders it when a navigation points somewhere; taking it writes
  /// no signal, so the render that takes it is not made to run again.
  pub fn take_pointed(&self, card: &Pointed) -> bool {
    let Some((number, pointed)) = self.pointed.get() else {
      return false;
    };
    pointed == *card && self.taken.fetch_max(number, Ordering::Relaxed) < number
  }

  /// Holds departures while a mounted editor has work it must not lose.
  pub fn guard(&self, holds_edits: impl Fn() -> bool + Send + Sync + 'static) -> guard::LeaveGuard {
    self.guard_within(holds_edits, |_| false)
  }

  /// Holds departures like [`guard`](Self::guard), but not a move in the same tab to a place `within` says
  /// is the editor's own (another of its panels), which stays on its edits.
  pub fn guard_within(
    &self,
    holds_edits: impl Fn() -> bool + Send + Sync + 'static,
    within: impl Fn(&Route) -> bool + Send + Sync + 'static,
  ) -> guard::LeaveGuard {
    let guard = guard::LeaveGuard::new(holds_edits, Box::new(within));
    *self.guard.lock() = guard.downgrade();
    guard
  }

  fn change(&self, change: impl FnOnce(&mut TabSet) -> bool) {
    let before = self.tabs_now();
    let mut next = before.clone();
    if !change(&mut next) {
      return;
    }
    let other_tab = before.active().id() != next.active().id();
    let leaving = other_tab || before.active().route() != next.active().route();
    let guard = self.guard.lock().upgrade();
    if leaving
      && guard.is_some_and(|guard| (other_tab || !guard.within(next.active().route())) && guard.hold(next.clone()))
    {
      return;
    }
    self.apply(next);
  }

  fn apply(&self, tabs: TabSet) {
    if self.pointed.with_untracked(Option::is_some) {
      self.pointed.set(None);
    }
    self.tabs.set(tabs);
    self.show();
  }

  fn show(&self) {
    let path = self.route_now().path();
    if self.router.path().get_untracked() != path {
      self.router.replace(path);
    }
  }
}
