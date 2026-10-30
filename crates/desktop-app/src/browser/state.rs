//! What the window's parts are drawn from: the project shown and its browser as its runner reports it, the strip of
//! its tabs, whether a browser was found, and where its requests go ([`Showing`]); and the streams and reads that keep
//! them current.

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, StreamEmitter},
  },
  core::ElementRef,
};
use orchester_core::id::RunId;
use orchester_ipc::browser::{BrowserMode, BrowserTabId, TabOwner};
use orchester_workspace::{
  browser::{BrowserView, BrowserWindow, RunOwner, run_owners},
  runner::Runners,
};

use super::{
  commands::Commands,
  link::{BrowserLink, Show},
  model::{Marks, Strip},
};
use crate::services::browser_lookup::BrowserLookup;

/// Whether the browser agents use was found on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
  Looking,
  Found,
  /// None found (`07` `YEv2c`).
  Missing,
}

impl DevtoolsInspectable for Lookup {}

/// The project the window shows, as its parts draw it.
#[derive(Clone)]
pub struct Showing {
  pub project: Arc<PathBuf>,
  pub name: Arc<str>,
  /// Its browser as its runner reported it; `None` until it did.
  pub view: Option<BrowserView>,
  /// Counts the changes of every project's browser.
  pub revision: u64,
  pub strip: Strip,
  pub found: Lookup,
  /// Its browser restarts at the person's request: the toolbar is faded and takes no input until the runner answers.
  pub restarting: bool,
  pub commands: Commands,
  /// The person's own use of the project's tabs.
  pub marks: Marks,
  /// What the person did in the window, which the root keeps.
  pub person: PersonHooks,
  /// The address line's text input, which Ctrl+L and the empty strip give the focus.
  pub address: ElementRef,
}

/// Two showings are the same while the project, its browser and what the window knows of it are.
impl PartialEq for Showing {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.name == other.name
      && self.revision == other.revision
      && self.view.is_some() == other.view.is_some()
      && self.strip == other.strip
      && self.found == other.found
      && self.restarting == other.restarting
      && self.marks == other.marks
  }
}

impl DevtoolsInspectable for Showing {}

impl Showing {
  /// The tab shown: screencast now, or only named with the browser in a window of its own (`05` `GjcAk`).
  pub fn shown(&self) -> Option<&BrowserTabId> {
    self.view.as_ref().and_then(BrowserView::shown)
  }

  pub fn mode(&self) -> BrowserMode {
    self.view.as_ref().map_or(BrowserMode::Headless, BrowserView::mode)
  }

  /// The browser cannot be used: no browser was found.
  pub fn unavailable(&self) -> bool {
    self.found == Lookup::Missing
  }

  /// The window drives the browser: one was found, it is not in a window of its own and it is not restarting.
  pub fn usable(&self) -> bool {
    !self.unavailable() && !self.restarting && self.mode() == BrowserMode::Headless
  }
}

type TabHook = Arc<dyn Fn(&BrowserTabId) + Send + Sync>;

/// What the window's parts tell the root about the person.
#[derive(Clone)]
pub struct PersonHooks {
  used: TabHook,
  pick: TabHook,
  look_again: Arc<dyn Fn() + Send + Sync>,
}

impl PersonHooks {
  pub fn new(
    used: impl Fn(&BrowserTabId) + Send + Sync + 'static,
    pick: impl Fn(&BrowserTabId) + Send + Sync + 'static,
    look_again: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      used: Arc::new(used),
      pick: Arc::new(pick),
      look_again: Arc::new(look_again),
    }
  }

  /// The person's input reached `tab`.
  pub fn used(&self, tab: &BrowserTabId) {
    (self.used)(tab);
  }

  /// The person chose `tab` to show: in the strip or the overflow menu.
  pub fn pick(&self, tab: &BrowserTabId) {
    (self.pick)(tab);
  }

  /// *Look again* for the browser.
  pub fn look_again(&self) {
    (self.look_again)();
  }
}

/// Each project's marks, by its folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarksByProject(pub HashMap<PathBuf, Marks>);

impl DevtoolsInspectable for MarksByProject {}

impl MarksByProject {
  pub fn of(&self, project: &Path) -> Marks {
    self.0.get(project).cloned().unwrap_or_default()
  }
}

/// The revision of every project's browser, whenever it changes.
pub fn follow_changes(ctx: &mut Ctx, window: &BrowserWindow) -> u64 {
  let window = window.clone();
  let changes = ctx.stream((), move |(), emitter: StreamEmitter<u64, ()>| {
    let mut updates = window.changes();
    async move {
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) || updates.changed().await.is_err() {
          return;
        }
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}

/// The latest request to show a project, whenever one is made.
pub fn follow_asked(ctx: &mut Ctx, link: &BrowserLink) -> Option<Show> {
  let link = link.clone();
  let asked = ctx.stream((), move |(), emitter: StreamEmitter<Option<Show>, ()>| {
    let mut asked = link.asked();
    async move {
      loop {
        let latest = asked.borrow_and_update().clone();
        if !emitter.emit(latest) || asked.changed().await.is_err() {
          return;
        }
      }
    }
  });
  asked.state().get().data.flatten()
}

/// The runs' work revision, which moves when a run starts, waits or ends: the tabs' owners are read again then.
pub fn follow_work(ctx: &mut Ctx, runners: &Runners) -> u64 {
  let changes = ctx.stream(runners.clone(), |runners, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = runners.work_changes();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}

/// Whether the browser was found, looked up off the UI thread; looked up again when `look` moves.
pub fn found(ctx: &mut Ctx, lookup: &BrowserLookup, look: u64) -> Lookup {
  let lookup = lookup.clone();
  let state = ctx
    .future(look, move |_| {
      let lookup = lookup.clone();
      async move {
        let found = tokio::task::spawn_blocking(move || lookup.find())
          .await
          .map_err(|error| error.to_string())?;
        Ok::<Lookup, String>(match found {
          Ok(program) => {
            tracing::debug!(browser = program.name(), "the Browser window found the browser");
            Lookup::Found
          }
          Err(unavailable) => {
            tracing::info!(reason = unavailable.reason(), "the Browser window found no browser");
            Lookup::Missing
          }
        })
      }
    })
    .state()
    .get();
  match (&state.data, &state.error) {
    (Some(found), _) => *found,
    (None, Some(error)) => {
      tracing::error!(%error, "looking for the browser failed");
      Lookup::Missing
    }
    (None, None) => Lookup::Looking,
  }
}

/// The owners of the run tabs of the project at `project`'s `view`, read off the UI thread, again when `work` moves.
pub fn owners(ctx: &mut Ctx, project: &Path, view: Option<&BrowserView>, work: u64) -> HashMap<RunId, RunOwner> {
  let mut runs: Vec<RunId> = view
    .map(|view| {
      view
        .tabs()
        .iter()
        .filter_map(|tab| match tab.owner {
          TabOwner::Run { run } => Some(run),
          TabOwner::Person => None,
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
    })
    .unwrap_or_default();
  runs.sort_unstable();
  let deps = (Owned(project.to_path_buf()), Owned(runs), work);
  let state = ctx
    .future(deps, |(Owned(project), Owned(runs), _)| async move {
      tokio::task::spawn_blocking(move || run_owners(&project, &runs).map_err(|error| error.to_string()))
        .await
        .map_err(|error| error.to_string())?
        .map(OwnersRead)
    })
    .state()
    .get();
  if let Some(error) = &state.error {
    tracing::warn!(project = %project.display(), %error, "the browser tabs' runs cannot be read");
  }
  state.data.map(|read| read.0).unwrap_or_default()
}

/// A value of a future's dependencies or result.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Owned<T>(T);

impl<T> DevtoolsInspectable for Owned<T> {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OwnersRead(HashMap<RunId, RunOwner>);

impl DevtoolsInspectable for OwnersRead {}
