//! The project's browser (`docs/design/browser.md`, decisions 3, 4 and 6, "The browser process"): one headless
//! Chromium per project, owned by its runner, the only process that talks to it ([`cdp`]). It starts on the first use
//! by any of the project's runs or by the person in the *Browser* window, on the project's own profile folder
//! ([`profile_of`]), and keeps running until the runner exits ([`ProjectBrowser::stop`]); one that ended on its own
//! starts again at the next use. Each tab a run opens belongs to that run, which reaches only its own tabs; they close
//! when the run ends (completed, cancelled, failed; a merged run had completed), and pausing keeps them. The tabs the
//! person opens in the window are the person's, and no run reaches them.
//!
//! The window sees and drives every tab through the runner (`window`): the browser's state, the screencast of the tab
//! it shows, the person's input, dialogs and `<select>` lists, and the restart in a browser window of its own.
//!
//! The browser ends with the runner: on Windows its kill-on-close Job Object ends it however the runner ends; on macOS
//! the runner closes it as it exits, and a runner killed with SIGKILL leaves it running, as it leaves agents
//! (`AGENTS.md`, rule 0).

pub mod cdp;
mod process;
mod running;
mod tabs;
mod window;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::id::RunId;
use orchester_exec::browser::BrowserUnavailable;
use orchester_ipc::browser::{AgentAction, BrowserEvent, BrowserMode, BrowserTabId, ScreencastFrame, TabOwner};
use orchester_store::ProjectDb;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard};

use self::{
  cdp::{CdpError, TargetId, TargetInfo},
  running::Running,
  window::Window,
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
  #[error("tab {0} is not one of this run's open tabs")]
  NotRunsTab(TargetId),
  #[error("the browser has no tab {0}")]
  NoSuchTab(TargetId),
  /// Back or forward with no page there in the tab's history.
  #[error("the tab has no page {0} in its history")]
  NoHistory(&'static str),
  /// A page's element, such as a `<select>` list, could not be read or set.
  #[error("the page's element: {0}")]
  Element(String),
}

/// The project's browser, started at its first use.
pub struct ProjectBrowser {
  profile: Result<PathBuf, BrowserUnavailable>,
  running: Mutex<Option<Running>>,
  window: Arc<Window>,
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
      running: Mutex::new(None),
      window: Arc::new(Window::default()),
    }
  }

  /// Opens a new tab for `run` on `url` and waits for its load, within a time limit. The tab is `run`'s from the
  /// moment it is attached: a load that fails leaves it to the run, which closes it or has it closed at its end. The
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

  /// Sends `method` with `params` to `tab`, one of `run`'s tabs; refused for any other tab, the person's included.
  pub async fn call(&self, run: RunId, tab: &TargetId, method: &str, params: Value) -> Result<Value, BrowserError> {
    let (cdp, session) = {
      let running = self.running.lock().await;
      running
        .as_ref()
        .and_then(|running| {
          let session = running.tabs.lock().session(run, tab)?;
          Some((Arc::clone(&running.cdp), session))
        })
        .ok_or_else(|| BrowserError::NotRunsTab(tab.clone()))?
    };
    Ok(cdp.call(Some(&session), method, params).await?)
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
    let owned = self
      .running
      .lock()
      .await
      .as_ref()
      .is_some_and(|running| running.tabs.lock().session(run, tab).is_some());
    if !owned {
      return Err(BrowserError::NotRunsTab(tab.clone()));
    }
    self.window.send(BrowserEvent::AgentActed {
      run,
      tab: BrowserTabId::new(tab.as_str()),
      action,
    });
    Ok(())
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
      *running = Some(Running::start(&profile, BrowserMode::Headless, &self.window).await?);
    }
    Ok(MutexGuard::map(running, |running| {
      running.as_mut().expect("a browser runs: it was kept or started above")
    }))
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
