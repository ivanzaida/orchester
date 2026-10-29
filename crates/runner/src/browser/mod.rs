//! The project's browser (`docs/design/browser.md`, decisions 3, 4 and 6, "The browser process"): one headless
//! Chromium per project, owned by its runner, the only process that talks to it ([`cdp`]). It starts on the first use
//! by any of the project's runs, on the project's own profile folder ([`profile_of`]), with the program the person
//! chose in Settings › Browser or else the one installed, and keeps running until the runner exits
//! ([`ProjectBrowser::stop`]); one that ended on its own starts again at the next use. Each tab a run opens belongs to
//! that run, which reaches only its own tabs; they close when the run ends (completed, cancelled, failed; a merged run
//! had completed), and pausing keeps them.
//!
//! The browser ends with the runner: on Windows its kill-on-close Job Object ends it however the runner ends; on macOS
//! the runner closes it as it exits, and a runner killed with SIGKILL leaves it running, as it leaves agents
//! (`AGENTS.md`, rule 0).

pub mod cdp;
mod process;
mod running;
mod tabs;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::id::RunId;
use orchester_exec::browser::{self as program, BrowserProgram, BrowserUnavailable};
use orchester_store::{AppDb, ProjectDb, location::browser_profiles_beside};
use parking_lot::Mutex;
use serde_json::Value;
use thiserror::Error;

use self::{
  cdp::{Cdp, CdpError, TargetId, TargetInfo},
  running::Running,
  tabs::RunTabs,
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
  #[error("tab {0} is not one of this run's open tabs")]
  NotRunsTab(TargetId),
}

/// The project's browser, started at its first use.
pub struct ProjectBrowser {
  profile: Result<PathBuf, BrowserUnavailable>,
  /// The application database whose Settings › Browser may name the program; `None`: the one installed.
  settings: Option<PathBuf>,
  running: tokio::sync::Mutex<Option<Running>>,
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
      running: tokio::sync::Mutex::new(None),
    }
  }

  /// Opens a new tab for `run` on `url` and waits for its load, within a time limit. The tab is `run`'s from the
  /// moment it is attached: a load that fails leaves it to the run, which closes it or has it closed at its end.
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
    tabs.lock().add(run, target.clone(), session.clone());
    running::load(&cdp, &session, url).await?;
    Ok(target)
  }

  /// Sends `method` with `params` to `tab`, one of `run`'s tabs; refused for any other tab.
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
    let settings = self.settings.clone();
    let found = tokio::task::spawn_blocking(move || chosen_or_found(settings.as_deref()))
      .await
      .map_err(|error| BrowserError::Start(format!("looking for the browser failed: {error}")))??;
    let started = Running::start(&found, &profile).await?;
    let handles = (Arc::clone(&started.cdp), Arc::clone(&started.tabs));
    *running = Some(started);
    Ok(handles)
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
