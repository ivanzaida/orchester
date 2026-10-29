//! The project's browser (`docs/design/browser.md`, decisions 3, 4 and 6, "The browser process"): one headless
//! Chromium per project, owned by its runner, the only process that talks to it ([`cdp`]). It starts on the first use
//! by any of the project's runs, on the project's own profile folder ([`profile_of`]), and keeps running until the
//! runner exits ([`ProjectBrowser::stop`]); one that ended on its own starts again at the next use. Each tab a run
//! opens belongs to that run, which reaches only its own tabs; they close when the run ends (completed, cancelled,
//! failed; a merged run had completed), and pausing keeps them.
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

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::id::RunId;
use orchester_exec::browser::BrowserUnavailable;
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use serde_json::{Value, json};
use thiserror::Error;

use self::{
  cdp::{Cdp, CdpError, TargetId, TargetInfo},
  running::Running,
  tab::TabState,
  tabs::RunTabs,
};
pub use self::{
  log::{ConsoleMessage, Request, TabLog},
  running::LOAD_LIMIT,
  tab::Tab,
};

#[derive(Debug, Error)]
pub enum BrowserError {
  /// No browser can be used: none is installed, or the profile folder is unknown.
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
  #[error("this run has no open tab: open a page with browser_open first")]
  NoTab,
  #[error("{0} is stale or was never read: read the page again with browser_read_page and use a ref it gives")]
  StaleRef(String),
}

/// What a running browser's connection listens to ([`ProjectBrowser::listening`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Listening {
  /// The receivers of its events still held: the runner's own of the browser's events, each open tab's log follower,
  /// and any wait in progress.
  pub receivers: usize,
  /// The tabs whose console messages, requests and dialogs are still followed.
  pub followed_logs: usize,
}

/// The project's browser, started at its first use.
pub struct ProjectBrowser {
  profile: Result<PathBuf, BrowserUnavailable>,
  running: tokio::sync::Mutex<Option<Running>>,
}

impl ProjectBrowser {
  /// The browser of the project at `project_root`, on its profile folder ([`profile_of`]).
  pub fn of_project(project_root: &Path) -> Self {
    Self::on(profile_of(project_root))
  }

  /// A browser on the profile folder `profile`.
  pub fn new(profile: PathBuf) -> Self {
    Self::on(Ok(profile))
  }

  fn on(profile: Result<PathBuf, BrowserUnavailable>) -> Self {
    Self {
      profile,
      running: tokio::sync::Mutex::new(None),
    }
  }

  /// Opens a new tab for `run` on `url` and waits for its load, within a time limit; it becomes the run's current
  /// tab. The tab is `run`'s from the moment it is attached: a load that fails leaves it to the run, which closes it
  /// or has it closed at its end. From then on the tab's console messages, requests and dialogs are recorded.
  pub async fn open_tab(&self, run: RunId, url: &str) -> Result<TargetId, BrowserError> {
    let (cdp, tabs) = self.connection().await?;
    let target = cdp.create_tab("about:blank").await?;
    let session = match cdp.attach(&target).await {
      Ok(session) => session,
      Err(error) => {
        if let Err(closing) = cdp.close_target(&target).await {
          tracing::warn!(%closing, "a tab that could not be attached was not closed");
        }
        return Err(error.into());
      }
    };
    let state = Arc::new(TabState::default());
    {
      let mut tabs = tabs.lock();
      tokio::spawn(log::follow(
        cdp.subscribe(Some(session.clone())),
        Arc::downgrade(&cdp),
        session.clone(),
        Arc::clone(&state),
        tabs.follower(),
      ));
      tabs.add(run, target.clone(), session.clone(), state);
    }
    for domain in ["Runtime.enable", "Network.enable", "Log.enable"] {
      cdp.call(Some(&session), domain, json!({})).await?;
    }
    running::load(&cdp, &session, url).await?;
    Ok(target)
  }

  /// `run`'s tab `tab`, or its current one for `None`; refused for a tab that is not the run's, and when the run has
  /// none open. Does not start the browser.
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
    let (session, state) = tabs
      .session(run, &target)
      .ok_or_else(|| BrowserError::NotRunsTab(target.clone()))?;
    Ok(Tab::new(target, Arc::clone(&running.cdp), session, state))
  }

  /// Sends `method` with `params` to `tab`, one of `run`'s tabs; refused for any other tab.
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

  /// Closes `tab`, one of `run`'s tabs, which is no longer the run's.
  pub async fn close_tab(&self, run: RunId, tab: &TargetId) -> Result<(), BrowserError> {
    let cdp = {
      let running = self.running.lock().await;
      let running = running.as_ref().ok_or_else(|| BrowserError::NotRunsTab(tab.clone()))?;
      let mut tabs = running.tabs.lock();
      if tabs.session(run, tab).is_none() {
        return Err(BrowserError::NotRunsTab(tab.clone()));
      }
      tabs.forget(tab);
      Arc::clone(&running.cdp)
    };
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
    let mut failed = None;
    for target in ended {
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

  /// Stops the browser, as the runner exits: `Browser.close`, so it saves its profile, then its process with
  /// everything it started once it has not exited within a grace period. Its tabs are gone; a later use starts it
  /// again.
  pub async fn stop(&self) {
    let running = self.running.lock().await.take();
    if let Some(running) = running {
      running.stop().await;
    }
  }

  /// The running browser's connection and tabs, starting it first when it does not run: its first use, or it ended on
  /// its own, with its tabs.
  async fn connection(&self) -> Result<(Arc<Cdp>, Arc<Mutex<RunTabs>>), BrowserError> {
    let mut running = self.running.lock().await;
    if let Some(current) = running.as_ref()
      && current.is_alive()
    {
      return Ok((Arc::clone(&current.cdp), Arc::clone(&current.tabs)));
    }
    if running.take().is_some() {
      tracing::warn!("the project's browser ended on its own; starting it again");
    }
    let profile = self.profile.clone()?;
    let started = Running::start(&profile).await?;
    let handles = (Arc::clone(&started.cdp), Arc::clone(&started.tabs));
    *running = Some(started);
    Ok(handles)
  }
}

/// The project's own browser profile folder: `<application data>/browser/<project id>/` beside `orchester.db`, never
/// the person's own browser profile and never inside a repository. Unavailable, with the reason, when either part
/// cannot be known.
pub fn profile_of(project_root: &Path) -> Result<PathBuf, BrowserUnavailable> {
  let location = orchester_store::location::app_data_location()
    .map_err(|error| BrowserUnavailable::new(format!("the application data folder is unknown: {error}")))?;
  let data = location
    .database
    .parent()
    .ok_or_else(|| BrowserUnavailable::new("the application database has no folder"))?;
  let project = ProjectDb::open_read_only(project_root)
    .map_err(|error| BrowserUnavailable::new(format!("the project cannot be read: {error}")))?
    .project()
    .id();
  Ok(data.join("browser").join(project.to_string()))
}
