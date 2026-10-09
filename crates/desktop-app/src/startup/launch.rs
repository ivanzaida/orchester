//! The session's start, before the window opens, and its quiet end once the window closed.

use std::{
  env,
  path::{Path, PathBuf},
};

use orchester_workspace::{
  app_paths,
  close::{CloseKind, session_end},
  runner::QUIET_STOP_TIMEOUT,
  session::{AppSession, StartupError},
  workspace::Workspace,
};
use tokio::runtime::Handle;

use super::{Problem, StartupProblem};
use crate::{
  project::{self, Landing},
  root::{Ready, Start},
  services::{Background, Services},
};

/// A started session with what runs for it, or why there is none.
pub struct Launch {
  start: Start,
  running: Option<Running>,
}

/// What a started session holds until it ends: the data folder's lock (in the session), and the
/// background tasks.
struct Running {
  session: AppSession,
  services: Services,
  _background: Background,
}

impl Launch {
  /// Starts the session of this build's data folder, with the runner program beside this one, on
  /// `runtime`. `log` is the app's log file, which a startup screen offers.
  pub fn begin(runtime: &Handle, log: Option<PathBuf>) -> Self {
    let located = app_paths::app_database_path()
      .and_then(|database| Ok((database, app_paths::runner_path()?)))
      .map_err(StartupError::ExecutableLocation);
    match located {
      Ok((database, runner)) => Self::begin_at(&database, runner, runtime, log),
      Err(error) => Self::problem(&error, log),
    }
  }

  /// Starts the session of the application database `database`, whose projects' runners are the
  /// program `runner`.
  pub fn begin_at(database: &Path, runner: PathBuf, runtime: &Handle, log: Option<PathBuf>) -> Self {
    let session = match AppSession::start_at(database) {
      Ok(session) => session,
      Err(error) => return Self::problem(&error, log),
    };
    let landing = match project::landing(session.workspace()) {
      Ok(landing) => landing,
      Err(source) => {
        let error = StartupError::Database {
          path: database.to_path_buf(),
          source,
        };
        return Self::problem(&error, log);
      }
    };
    let services = Services::start(&session, runner, runtime.clone());
    let background = services.run_background();
    Self {
      start: Start::Ready(Box::new(Ready {
        services: services.clone(),
        landing,
      })),
      running: Some(Running {
        session,
        services,
        _background: background,
      }),
    }
  }

  fn problem(error: &StartupError, log: Option<PathBuf>) -> Self {
    tracing::error!(%error, "cannot start the session");
    let program = env::current_exe().ok();
    let program_folder = program.as_deref().and_then(Path::parent);
    Self {
      start: Start::Problem(Problem {
        problem: StartupProblem::of(error, program_folder),
        log,
      }),
      running: None,
    }
  }

  /// What the window shows.
  pub fn start(&self) -> Start {
    self.start.clone()
  }

  /// Where the session opens; `None` without a session.
  pub fn landing(&self) -> Option<&Landing> {
    match &self.start {
      Start::Ready(ready) => Some(&ready.landing),
      Start::Problem(_) => None,
    }
  }

  /// The session's workspace, which keeps the app's settings; `None` without a session.
  pub fn workspace(&self) -> Option<&Workspace> {
    self.running.as_ref().map(|running| &running.services.workspace)
  }

  pub fn services(&self) -> Option<&Services> {
    self.running.as_ref().map(|running| &running.services)
  }

  /// Ends the session quietly once the window closed: every runner is asked to stop at a checkpoint,
  /// waiting at most [`QUIET_STOP_TIMEOUT`] for each, and how the session ended is recorded. Without a
  /// session there is nothing to end.
  pub fn end(self) {
    let Some(Running { session, services, .. }) = self.running else {
      return;
    };
    let outcome = services
      .runtime
      .block_on(services.runners.shutdown(Some(QUIET_STOP_TIMEOUT)));
    let end = session_end(CloseKind::Quiet, outcome);
    tracing::info!(?outcome, ?end, "the session ends");
    if let Err(error) = session.end(end) {
      tracing::error!(%error, "cannot record the end of the session");
    }
  }
}
