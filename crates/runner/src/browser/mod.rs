//! The project's browser (`docs/design/browser.md`, decisions 3, 4 and 6, "The browser process"): one headless
//! Chromium per project, owned by its runner, the only process that talks to it ([`cdp`]). It starts on the first use
//! by any of the project's runs or by the person in the *Browser* window, on the project's own profile folder
//! ([`profile_of`]), with the program the person chose in Settings › Browser or else the one installed, and keeps
//! running until the runner exits ([`ProjectBrowser::stop`]); one that ended on its own starts again at the next use.
//! Each tab a run opens belongs to that run, which reaches only its own tabs; they close when the run ends (completed,
//! cancelled, failed; a merged run had completed), and pausing keeps them. The tabs the person opens in the window are
//! the person's, and no run reaches them.
//!
//! The window sees and drives every tab through the runner (`window`): the browser's state, the screencast of the tab
//! it shows, the person's input, dialogs and `<select>` lists, and the restart in a browser window of its own.
//!
//! The browser ends with the runner: on Windows its kill-on-close Job Object ends it however the runner ends; on macOS
//! the runner closes it as it exits, and a runner killed with SIGKILL leaves it running, as it leaves agents
//! (`AGENTS.md`, rule 0).

pub mod cdp;
mod log;
mod process;
mod running;
mod tab;
mod tabs;
mod window;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::id::RunId;
use orchester_exec::browser::{self as program, BrowserProgram, BrowserUnavailable};
use orchester_ipc::browser::{AgentAction, BrowserEvent, BrowserMode, BrowserTabId, ScreencastFrame, TabOwner};
use orchester_store::{AppDb, ProjectDb, location::browser_profiles_beside};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard};

use self::{
  cdp::{CdpError, TargetId, TargetInfo},
  running::Running,
  window::Window,
};
pub use self::{
  log::{ConsoleMessage, Request, TabLog},
  running::LOAD_LIMIT,
  tab::Tab,
};

#[derive(Debug, Error)]
pub enum BrowserError {
  /// No browser can be used: none is installed, the one chosen cannot be used, or the profile folder is unknown.
  #[error(transparent)]
  Unavailable(#[from] BrowserUnavailable),
  #[error("the browser could not start: {0}")]
  Start(String),
  #[error(transparent)]
  Cdp(#[from] CdpError),
  #[error("{url} did not open: {reason}")]
  NotOpened { url: String, reason: String },
  #[error("tab {0} is not one of this run's open tabs: browser_tabs lists them")]
  NotRunsTab(TargetId),
  /// One of the run's tabs the person closed from the *Browser* window.
  #[error("the person closed tab {0} in the Browser window: open a new one with browser_open if you still need one")]
  ClosedByPerson(TargetId),
  /// One of the run's tabs the browser reported closed, which neither the run nor the *Browser* window closed.
  #[error(
    "tab {0} was closed in the browser, by its own page or by the person in the browser's window: open a new one \
     with browser_open if you still need one"
  )]
  ClosedInBrowser(TargetId),
  #[error("this run has no open tab: open a page with browser_open first")]
  NoTab,
  #[error("{0} is stale or was never read: read the page again with browser_read_page and use a ref it gives")]
  StaleRef(String),
  #[error("the browser has no tab {0}")]
  NoSuchTab(TargetId),
  /// Back or forward with no page there in the tab's history.
  #[error("the tab has no page {0} in its history")]
  NoHistory(&'static str),
  /// A page's element, such as a `<select>` list, could not be read or set.
  #[error("the page's element: {0}")]
  Element(String),
}

/// What a running browser's connection listens to ([`ProjectBrowser::listening`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Listening {
  /// The receivers of its events still held: the runner's own of the browser's events, each open tab's followers
  /// (the window's, and a run's tab's log), and any wait in progress.
  pub receivers: usize,
  /// The tabs whose console messages, requests and dialogs are still followed.
  pub followed_logs: usize,
}

/// The project's browser, started at its first use.
pub struct ProjectBrowser {
  profile: Result<PathBuf, BrowserUnavailable>,
  /// The application database whose Settings › Browser may name the program; `None`: the one installed.
  settings: Option<PathBuf>,
  running: Mutex<Option<Running>>,
  window: Arc<Window>,
}

impl ProjectBrowser {
  /// The browser of the project at `project_root`, on its profile folder ([`profile_of`]), with the program Settings ›
  /// Browser names in the application database beside the runner.
  pub fn of_project(project_root: &Path) -> Self {
    let profile = profile_of(project_root);
    // Without the application database the profile folder is unknown too, and its error says why.
    let settings = app_database().ok();
    Self {
      settings,
      ..Self::on(profile)
    }
  }

  /// A browser on the profile folder `profile`, with the program installed.
  pub fn new(profile: PathBuf) -> Self {
    Self::on(Ok(profile))
  }

  /// This browser with the program Settings › Browser names in the application database `database`.
  #[must_use]
  pub fn with_settings(mut self, database: PathBuf) -> Self {
    self.settings = Some(database);
    self
  }

  fn on(profile: Result<PathBuf, BrowserUnavailable>) -> Self {
    Self {
      profile,
      settings: None,
      running: Mutex::new(None),
      window: Arc::new(Window::default()),
    }
  }

  /// Opens a new tab for `run` on `url` and waits for its load, within a time limit; it becomes the run's current
  /// tab. The tab is `run`'s from the moment it is attached: a load that fails leaves it to the run, which closes it
  /// or has it closed at its end. From then on the tab's console messages, requests and dialogs are recorded. The
  /// run's first tab tells the window that an agent started browsing.
  pub async fn open_tab(&self, run: RunId, url: &str) -> Result<TargetId, BrowserError> {
    let (target, session, cdp, first) = {
      let running = self.connection().await?;
      let first = running.tabs.lock().of(run).is_empty();
      let (target, session) = running.open(TabOwner::Run { run }).await?;
      (target, session, Arc::clone(&running.cdp), first)
    };
    if first {
      self.window.send(BrowserEvent::AgentStartedBrowsing { run });
    }
    running::load(&cdp, &session, url).await?;
    Ok(target)
  }

  /// `run`'s tab `tab`, or its current one for `None`; refused for a tab that is not the run's, the person's included,
  /// for one someone other than the run closed, saying who, and when the run has none open. Does not start the
  /// browser.
  pub async fn tab(&self, run: RunId, tab: Option<&TargetId>) -> Result<Tab, BrowserError> {
    let running = self.running.lock().await;
    let Some(running) = running.as_ref() else {
      return Err(tab.map_or(BrowserError::NoTab, |tab| BrowserError::NotRunsTab(tab.clone())));
    };
    let tabs = running.tabs.lock();
    let target = match tab {
      Some(tab) => tab.clone(),
      None => tabs.current(run).ok_or(BrowserError::NoTab)?,
    };
    let (session, state) = tabs.session(run, &target).ok_or_else(|| tabs.refused(run, &target))?;
    Ok(Tab::new(target, Arc::clone(&running.cdp), session, state))
  }

  /// Sends `method` with `params` to `tab`, one of `run`'s tabs; refused for any other tab, the person's included.
  pub async fn call(&self, run: RunId, tab: &TargetId, method: &str, params: Value) -> Result<Value, BrowserError> {
    Ok(self.tab(run, Some(tab)).await?.call(method, params).await?)
  }

  /// Loads `url` in `tab`, one of `run`'s tabs, which becomes its current one, and waits for the load.
  pub async fn load(&self, run: RunId, tab: &Tab, url: &str) -> Result<(), BrowserError> {
    self.make_current(run, tab.id()).await;
    tab.load(url).await
  }

  /// `run`'s current tab, which a call that names none acts on; `None` while it has none open.
  pub async fn current(&self, run: RunId) -> Option<TargetId> {
    let running = self.running.lock().await;
    running.as_ref().and_then(|running| running.tabs.lock().current(run))
  }

  async fn make_current(&self, run: RunId, tab: &TargetId) {
    if let Some(running) = self.running.lock().await.as_ref() {
      running.tabs.lock().make_current(run, tab);
    }
  }

  /// Closes `tab`, one of `run`'s tabs, which is no longer the run's; the window no longer shows it.
  pub async fn close_tab(&self, run: RunId, tab: &TargetId) -> Result<(), BrowserError> {
    let cdp = {
      let running = self.running.lock().await;
      let running = running.as_ref().ok_or_else(|| BrowserError::NotRunsTab(tab.clone()))?;
      let mut tabs = running.tabs.lock();
      if tabs.session(run, tab).is_none() {
        return Err(tabs.refused(run, tab));
      }
      tabs.forget(tab);
      Arc::clone(&running.cdp)
    };
    self.window.forget(tab);
    self.window.changed();
    Ok(cdp.close_target(tab).await?)
  }

  /// `run`'s open tabs, in the order they opened; none while the browser does not run, which this does not start.
  pub async fn tabs(&self, run: RunId) -> Result<Vec<TargetInfo>, BrowserError> {
    let (cdp, own) = {
      let running = self.running.lock().await;
      let Some(running) = running.as_ref() else {
        return Ok(Vec::new());
      };
      (Arc::clone(&running.cdp), running.tabs.lock().of(run))
    };
    if own.is_empty() {
      return Ok(Vec::new());
    }
    let mut listed = cdp.targets().await?;
    listed.retain(|target| own.contains(&target.id));
    listed.sort_by_key(|target| own.iter().position(|id| id == &target.id));
    Ok(listed)
  }

  /// Tells the window what an agent of `run` did on `tab`, one of the run's tabs, for *Agent is acting*: the browser
  /// tools report each action so, not CDP.
  pub async fn agent_acted(&self, run: RunId, tab: &TargetId, action: AgentAction) -> Result<(), BrowserError> {
    {
      let running = self.running.lock().await;
      let running = running.as_ref().ok_or_else(|| BrowserError::NotRunsTab(tab.clone()))?;
      let tabs = running.tabs.lock();
      if tabs.session(run, tab).is_none() {
        return Err(tabs.refused(run, tab));
      }
    }
    self.window.send(BrowserEvent::AgentActed {
      run,
      tab: BrowserTabId::new(tab.as_str()),
      action,
    });
    Ok(())
  }

  /// What the running browser's connection still listens to, so that closed tabs are seen to leave nothing behind;
  /// nothing while it does not run.
  pub async fn listening(&self) -> Listening {
    let running = self.running.lock().await;
    running.as_ref().map_or_else(Listening::default, |running| Listening {
      receivers: running.cdp.listeners(),
      followed_logs: running.tabs.lock().followed(),
    })
  }

  /// `run` ended: its tabs are no longer its and close. Every tab is tried; the first failure is returned.
  pub async fn close_run_tabs(&self, run: RunId) -> Result<(), BrowserError> {
    let (cdp, ended) = {
      let running = self.running.lock().await;
      let Some(running) = running.as_ref() else {
        return Ok(());
      };
      (Arc::clone(&running.cdp), running.tabs.lock().take(run))
    };
    if !ended.is_empty() {
      self.window.changed();
    }
    let mut failed = None;
    for target in ended {
      self.window.forget(&target);
      if let Err(error) = cdp.close_target(&target).await {
        failed.get_or_insert(error);
      }
    }
    failed.map_or(Ok(()), |error| Err(error.into()))
  }

  /// [`ProjectBrowser::close_run_tabs`] on a task of its own, for the scheduler as a run ends; a failure is logged.
  pub fn end_run(self: &Arc<Self>, run: RunId) {
    let browser = Arc::clone(self);
    tokio::spawn(async move {
      if let Err(error) = browser.close_run_tabs(run).await {
        tracing::warn!(%run, %error, "the ended run's tabs could not all be closed");
      }
    });
  }

  /// The latest frame of the tab the window shows, once there is one: a newer frame replaced every frame before it
  /// that was not taken yet.
  pub async fn next_frame(&self) -> ScreencastFrame {
    self.window.next_frame().await
  }

  /// How many frames a newer one replaced before they were taken.
  pub fn dropped_frames(&self) -> u64 {
    self.window.dropped_frames()
  }

  /// Stops the browser, as the runner exits: `Browser.close`, so it saves its profile, then its process with
  /// everything it started once it has not exited within a grace period. Its tabs are gone; a later use starts it
  /// again.
  pub async fn stop(&self) {
    let running = self.running.lock().await.take();
    if let Some(running) = running {
      running.stop().await;
    }
  }

  /// The running browser, started first when it does not run: its first use, or it ended on its own, with its tabs.
  /// One that starts so is headless.
  async fn connection(&self) -> Result<MappedMutexGuard<'_, Running>, BrowserError> {
    let mut running = self.running.lock().await;
    if running.as_ref().is_some_and(|current| !current.is_alive()) {
      tracing::warn!("the project's browser ended on its own; starting it again");
      running.take();
    }
    if running.is_none() {
      let profile = self.profile.clone()?;
      let found = self.program().await?;
      *running = Some(Running::start(&found, &profile, BrowserMode::Headless, &self.window).await?);
    }
    Ok(MutexGuard::map(running, |running| {
      running.as_mut().expect("a browser runs: it was kept or started above")
    }))
  }

  /// The program the browser starts with: the one chosen in Settings › Browser, else the one installed
  /// ([`chosen_or_found`]).
  async fn program(&self) -> Result<BrowserProgram, BrowserError> {
    let settings = self.settings.clone();
    Ok(
      tokio::task::spawn_blocking(move || chosen_or_found(settings.as_deref()))
        .await
        .map_err(|error| BrowserError::Start(format!("looking for the browser failed: {error}")))??,
    )
  }
}

/// The project's own browser profile folder: `<application data>/browser/<project id>/` beside `orchester.db`, never
/// the person's own browser profile and never inside a repository. Unavailable, with the reason, when either part
/// cannot be known.
pub fn profile_of(project_root: &Path) -> Result<PathBuf, BrowserUnavailable> {
  let profiles = browser_profiles_beside(&app_database()?)
    .ok_or_else(|| BrowserUnavailable::new("the application database has no folder"))?;
  let project = ProjectDb::open_read_only(project_root)
    .map_err(|error| BrowserUnavailable::new(format!("the project cannot be read: {error}")))?
    .project()
    .id();
  Ok(profiles.join(project.to_string()))
}

/// The application database beside the runner, as the app that started it keeps it.
fn app_database() -> Result<PathBuf, BrowserUnavailable> {
  orchester_store::location::app_data_location()
    .map(|location| location.database)
    .map_err(|error| BrowserUnavailable::new(format!("the application data folder is unknown: {error}")))
}

/// The program Settings › Browser names in the application database `settings` when the person chose one, else the
/// one installed; `None` (a test's browser) or a database not made yet is the one installed. A chosen program that
/// is no Chromium-based browser's (any more) is unavailable, saying so: the person's choice is never passed over.
/// Blocks on SQLite and, on Windows, on the registry.
fn chosen_or_found(settings: Option<&Path>) -> Result<BrowserProgram, BrowserUnavailable> {
  let chosen = match settings {
    Some(database) if database.is_file() => AppDb::open_read_only(database)
      .and_then(|settings| settings.browser_program())
      .map_err(|error| BrowserUnavailable::new(format!("Settings › Browser cannot be read: {error}")))?,
    _ => None,
  };
  match chosen {
    Some(program) => program::chosen(&program).map_err(|refused| {
      BrowserUnavailable::new(format!(
        "the program chosen in Settings › Browser cannot be used: {refused}"
      ))
    }),
    None => program::find(),
  }
}
