//! The session's start, before the window opens, and its quiet end once the window closed.

use std::{
  env, fs,
  path::{Path, PathBuf},
};

use orchester_store::SessionEnd;
use orchester_workspace::{
  app_paths,
  close::{CloseKind, session_end},
  runner::QUIET_STOP_TIMEOUT,
  secrets::Vault,
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
  background: Background,
}

impl Launch {
  /// Starts the session of this build's data folder, with the runner program beside this one, on
  /// `runtime`. `log` is the app's log file, which a startup screen offers.
  pub fn begin(runtime: &Handle, log: Option<PathBuf>) -> Self {
    let located =
      app_paths::app_data_location().and_then(|location| Ok((location.database, app_paths::runner_path()?)));
    let (database, runner) = match located {
      Ok(located) => located,
      Err(error) => return Self::problem(&StartupError::ExecutableLocation(error), log),
    };
    // The folder of a first start (the Library folder of a macOS app bundle) is made here; one that
    // cannot be made is a folder that cannot be written, not a program that cannot find itself.
    if let Some(folder) = database.parent()
      && let Err(source) = fs::create_dir_all(folder)
    {
      let folder = folder.to_path_buf();
      return Self::problem(&StartupError::DataFolder { folder, source }, log);
    }
    Self::begin_at(&database, runner, runtime, log)
  }

  /// Starts the session of the application database `database`, whose projects' runners are the
  /// program `runner`, with the OS keychain.
  pub fn begin_at(database: &Path, runner: PathBuf, runtime: &Handle, log: Option<PathBuf>) -> Self {
    Self::begin_with_vault(database, runner, runtime, (log, Vault::keychain()))
  }

  /// [`Self::begin_at`] with `vault` for secrets and tokens, e.g. a keychain in memory in a test.
  pub fn begin_with_vault(
    database: &Path,
    runner: PathBuf,
    runtime: &Handle,
    (log, vault): (Option<PathBuf>, Vault),
  ) -> Self {
    let session = match AppSession::start_at(database) {
      Ok(session) => session,
      Err(error) => return Self::problem(&error, log),
    };
    let landing = match project::landing(session.workspace()) {
      Ok(landing) => landing,
      Err(source) => {
        // The session was recorded; it ends here, so the next start does not take it for a crash.
        if let Err(error) = session.end(SessionEnd::Clean) {
          tracing::error!(%error, "cannot record the end of a session that could not open");
        }
        let error = StartupError::Database {
          path: database.to_path_buf(),
          source,
        };
        return Self::problem(&error, log);
      }
    };
    let services = Services::start(&session, runner, runtime.clone(), vault);
    let background = services.run_background();
    Self {
      start: Start::Ready(Box::new(Ready {
        services: services.clone(),
        landing,
      })),
      running: Some(Running {
        session,
        services,
        background,
      }),
    }
  }

  fn problem(error: &StartupError, log: Option<PathBuf>) -> Self {
    match error {
      StartupError::InUse { .. } => tracing::warn!(%error, "another app has the data folder open"),
      error => tracing::error!(%error, "cannot start the session"),
    }
    // The program's folder only picks the not-writable screen's words (data beside the program or in a
    // folder elsewhere); without it the screen words it as a folder elsewhere.
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

  /// Ends the session once the window closed. Its lock says it is closing; the shared servers' monitor
  /// and sync stop; the close flow has stopped the runners as the person chose (otherwise every runner
  /// is asked to stop at a checkpoint, waiting at most [`QUIET_STOP_TIMEOUT`] for each); the services
  /// go; and how the session ended is recorded, which lets go of the data folder last. Without a
  /// session there is nothing to end.
  pub fn end(self) {
    let Some(Running {
      mut session,
      services,
      background,
    }) = self.running
    else {
      return;
    };
    session.mark_closing();
    drop(background);
    let end = services.close.session_end().unwrap_or_else(|| {
      let outcome = services
        .runtime
        .block_on(services.runners.shutdown(Some(QUIET_STOP_TIMEOUT)));
      session_end(CloseKind::Quiet, outcome)
    });
    drop(services);
    tracing::info!(?end, "the session ends");
    if let Err(error) = session.end(end) {
      tracing::error!(%error, "cannot record the end of the session");
    }
  }
}
