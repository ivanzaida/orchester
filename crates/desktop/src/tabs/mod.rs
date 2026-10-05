//! The window's browser-like tabs (design doc, "Tabs"; title bar tabs `nAKyb` in `QQmsI` and
//! `V70DN`). A tab is a location — a project and a page, or All projects — with its own
//! Back/Forward history; the sidebar, the page and the status bar follow the active tab.
//!
//! Each tab has a [`host::TabHost`] with a router of its own, mounted offstage while the tab is
//! inactive: lurq keeps its components, signals, scroll and drafts, but leaves it out of layout,
//! painting, input, timers and future polling until it is active again. [`Tabs`] is the active
//! tab navigator: the application's history input, the shortcuts and every "open in a new tab"
//! go through it, and it acts on the active tab's router. Data that belongs to a project rather
//! than a tab (its tasks, usage, runner status) is read once per project for every tab of it
//! ([`feed`]).

pub mod dirty;
pub mod feed;
mod host;
pub mod input;
pub mod label;
pub mod model;
mod persist;
pub mod restore;
mod shared;
pub mod strip;
pub mod window;

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::events::{MouseButton, MouseEvent},
  core::Signal,
  router::RouterHandle,
};
use parking_lot::Mutex;

use self::{
  dirty::{Discard, Leaving, Unsaved},
  model::{Closed, Focus, TabId, TabSet},
  persist::Saver,
};
use crate::{
  navigation::{Places, entry, forward, route::Route},
  ui::backdrop::DialogLayer,
  workspace::{OpenProject, Workspace},
};

/// Where a place opens: the current tab, or a new tab right of it, in the background or active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenIn {
  Current,
  Background,
  NewTab,
}

impl OpenIn {
  /// How a pointer click opens a place: Ctrl/Cmd+click and a middle click in a background tab,
  /// Ctrl/Cmd+Shift+click in a new active tab, any other click in the current tab.
  pub fn of_click(event: &MouseEvent, command: bool) -> Self {
    if event.button == MouseButton::Middle {
      return Self::Background;
    }
    match (event.ctrl || command, event.shift) {
      (true, true) => Self::NewTab,
      (true, false) => Self::Background,
      (false, _) => Self::Current,
    }
  }
}

/// What the window last saw of the keys and the pointer, to tell how a click on a link asks to
/// open its place.
#[derive(Clone, Copy, Default)]
struct PointerInput {
  /// Whether Cmd is held (macOS): lurq's mouse events carry Ctrl, Shift and Alt only, so the
  /// window follows the key's presses and releases to tell a Cmd+click.
  command: bool,
  /// Whether the pointer pressed a link since Enter or Space last went down. lurq presses a
  /// focused control for Enter and Space with a click of its own, after the key press and without
  /// a pointer press.
  pointer: bool,
}

/// A project opened from a list, and where it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenedProject {
  pub project: OpenProject,
  pub open: OpenIn,
}

impl lurq::app::component::DevtoolsInspectable for OpenedProject {}

/// The window's tabs and the active tab navigator, provided to every component by the window.
#[derive(Clone)]
pub struct Tabs {
  inner: Arc<Inner>,
}

struct Inner {
  set: Signal<TabSet>,
  places: Places,
  workspace: Workspace,
  /// Each open tab's router, registered by its host.
  routers: Mutex<HashMap<TabId, RouterHandle>>,
  unsaved: Unsaved,
  /// What the discard question asks about while it is open: closing a tab, or leaving a page.
  discarding: Signal<Option<Discard>>,
  dialogs: DialogLayer,
  saver: Saver,
  input: Mutex<PointerInput>,
}

impl PartialEq for Tabs {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl lurq::app::component::DevtoolsInspectable for Tabs {}

impl Tabs {
  /// The tabs of `set`; their changes are saved on `runtime`.
  pub fn new(set: TabSet, places: Places, workspace: Workspace, runtime: tokio::runtime::Handle) -> Self {
    let saver = Saver::new(workspace.clone(), runtime);
    saver.save(&set);
    Self {
      inner: Arc::new(Inner {
        set: Signal::new(set),
        places,
        workspace,
        routers: Mutex::new(HashMap::new()),
        unsaved: Unsaved::default(),
        discarding: Signal::new(None),
        dialogs: DialogLayer::default(),
        saver,
        input: Mutex::new(PointerInput::default()),
      }),
    }
  }

  /// The tabs, read reactively.
  pub fn set(&self) -> &Signal<TabSet> {
    &self.inner.set
  }

  pub fn places(&self) -> &Places {
    &self.inner.places
  }

  pub fn workspace(&self) -> &Workspace {
    &self.inner.workspace
  }

  pub fn dialogs(&self) -> &DialogLayer {
    &self.inner.dialogs
  }

  pub fn unsaved(&self) -> &Unsaved {
    &self.inner.unsaved
  }

  /// What the discard question asks about, while it is open.
  pub fn discarding(&self) -> &Signal<Option<Discard>> {
    &self.inner.discarding
  }

  /// Goes where the user asked in the active tab (`go`), after asking when its page holds edits
  /// that ask before it is left (`kDmo9`). Only the active tab takes input, so every user
  /// navigation is the active tab's.
  pub fn leave_active(&self, go: impl Fn() + Send + Sync + 'static) {
    let active = self.inner.set.get_untracked().active();
    match self.inner.unsaved.leave_question(active) {
      Some(explanation) => self
        .inner
        .discarding
        .set(Some(Discard::Leave(Leaving::new(explanation, Arc::new(go))))),
      None => go(),
    }
  }

  fn change(&self, change: impl FnOnce(&mut TabSet) -> bool) -> bool {
    let mut set = self.inner.set.get_untracked();
    if !change(&mut set) {
      return false;
    }
    self.inner.saver.save(&set);
    self.inner.set.set(set);
    true
  }

  /// A new tab on All projects, right of the active one, active (+, Ctrl/Cmd+T).
  pub fn new_tab(&self) {
    self.change(|set| {
      set.open_home();
      true
    });
  }

  /// Opens `route` as `open` says; in the current tab it is a new history entry.
  pub fn open(&self, route: Route, open: OpenIn) {
    self.open_path(route.path(), open);
  }

  /// Opens the place `path` names as `open` says, for a path that carries more than its route
  /// (a task's Changes tab on a file).
  pub fn open_path(&self, path: String, open: OpenIn) {
    match open {
      OpenIn::Current => {
        if let Some(router) = self.active_router() {
          self.leave_active(move || entry::push(&router.navigator(), path.clone()));
        }
      }
      OpenIn::Background => self.open_tab(&path, Focus::Background),
      OpenIn::NewTab => self.open_tab(&path, Focus::Switch),
    }
  }

  fn open_tab(&self, path: &str, focus: Focus) {
    self.change(|set| {
      set.open(path, focus);
      true
    });
  }

  pub fn activate(&self, id: TabId) {
    self.change(|set| set.activate(id));
  }

  /// Closes `id` (×, middle click, Ctrl/Cmd+W), after asking when it holds unsaved edits.
  pub fn request_close(&self, id: TabId) {
    if !self.inner.set.get_untracked().closable(id) {
      return;
    }
    if self.inner.unsaved.any(id) {
      self.inner.discarding.set(Some(Discard::Close(id)));
      return;
    }
    self.close(id);
  }

  /// Closes the active tab, as Ctrl/Cmd+W does.
  pub fn close_active(&self) {
    self.request_close(self.inner.set.get_untracked().active());
  }

  /// Closes `id` without asking; its history and edits are gone.
  pub fn close(&self, id: TabId) {
    let mut closed = Closed::Kept;
    self.change(|set| {
      closed = set.close(id);
      closed != Closed::Kept
    });
    if closed != Closed::Kept {
      self.inner.routers.lock().remove(&id);
      self.inner.unsaved.forget(id);
    }
  }

  pub fn next(&self) {
    self.change(TabSet::activate_next);
  }

  pub fn previous(&self) {
    self.change(TabSet::activate_previous);
  }

  /// Activates tab `index` (from 0).
  pub fn select(&self, index: usize) {
    self.change(|set| set.select(index));
  }

  pub fn select_last(&self) {
    self.change(TabSet::select_last);
  }

  /// Moves `id` to `index` among the tabs.
  pub fn reorder(&self, id: TabId, index: usize) {
    self.change(|set| set.move_tab(id, index));
  }

  /// Back in the active tab's history; nothing at its start.
  pub fn back(&self) {
    if let Some(router) = self.active_router().filter(RouterHandle::can_back) {
      self.leave_active(move || {
        router.back();
      });
    }
  }

  /// Forward in the active tab's history; nothing at its end.
  pub fn forward(&self) {
    if let Some(router) = self.active_router().filter(RouterHandle::can_forward) {
      self.leave_active(move || {
        forward(&router);
      });
    }
  }

  fn active_router(&self) -> Option<RouterHandle> {
    let active = self.inner.set.get_untracked().active();
    self.inner.routers.lock().get(&active).cloned()
  }

  /// Registered by a tab's host when it mounts.
  fn register(&self, id: TabId, router: RouterHandle) {
    self.inner.routers.lock().insert(id, router);
  }

  /// A tab's router shows `path`: the tab records it, and the window saves it.
  fn sync(&self, id: TabId, path: &str) {
    self.change(|set| set.set_path(id, path));
  }

  /// How `event`, a click on a link, asks to open its place (see [`OpenIn::of_click`]). Cmd
  /// counts as the window last saw it, and only on macOS, where it stands for Ctrl. A click that
  /// Enter or Space made on a focused control opens in the current tab, whatever keys are held.
  pub fn open_in(&self, event: &MouseEvent) -> OpenIn {
    let input = *self.inner.input.lock();
    if event.button == MouseButton::Left && !input.pointer {
      return OpenIn::Current;
    }
    OpenIn::of_click(event, cfg!(target_os = "macos") && input.command)
  }

  /// The pointer pressed a link.
  pub fn pointer_pressed(&self) {
    self.inner.input.lock().pointer = true;
  }

  /// A key went down with Cmd held or not. After Enter or Space (`activates`), a click without a
  /// pointer press is the keyboard's; other keys, the modifiers repeating while held among them,
  /// leave a pointer press as it is.
  fn key_pressed(&self, command: bool, activates: bool) {
    let mut input = self.inner.input.lock();
    input.command = command;
    if activates {
      input.pointer = false;
    }
  }

  fn key_released(&self, command: bool) {
    self.inner.input.lock().command = command;
  }
}
