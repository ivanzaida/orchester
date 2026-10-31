//! The session's start, before the window opens, and its quiet end once the window closed.

use std::{
  env, fs,
  path::{Path, PathBuf},
};

use orchester_exec::programs::Programs;
use orchester_store::SessionEnd;
use orchester_workspace::{
  app_paths,
  app_updates::{self, CHECK_EVERY},
  close::{CloseKind, session_end},
  runner::QUIET_STOP_TIMEOUT,
  secrets::Vault,
  session::{AppSession, StartupError},
  tasks::resolve::Editor,
  workspace::Workspace,
};
use tokio::runtime::Handle;

use super::{Problem, StartupProblem};
use crate::{
  open_link::{self, OpenLink},
  project::{self, Landing},
  root::{Ready, Start},
  services::{Background, Services},
};

/// A started session with what runs for it, or why there is none.
pub struct Launch {
  start: Start,
  running: Option<Running>,
}

/// What a started session holds until it ends: the data folder's lock (in the session), the data folder, and the
/// background tasks.
struct Running {
  session: AppSession,
  folder: PathBuf,
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
        folder: database.parent().unwrap_or(Path::new("")).to_path_buf(),
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

  /// The session with the window's git and `gh` run by `programs`, e.g. captured answers in a test, so it never
  /// reaches the person's GitHub.
  #[must_use]
  pub fn with_programs(mut self, programs: Programs) -> Self {
    if let Start::Ready(ready) = &mut self.start {
      ready.services.programs = programs.clone();
    }
    if let Some(running) = &mut self.running {
      running.services.programs = programs;
    }
    self
  }

  /// The session with *Resolve in editor* starting `editor`, e.g. a stand-in in a test, so no editor opens on the
  /// person's desktop.
  #[must_use]
  pub fn with_editor(mut self, editor: Option<Editor>) -> Self {
    if let Start::Ready(ready) = &mut self.start {
      ready.services.editor.clone_from(&editor);
    }
    if let Some(running) = &mut self.running {
      running.services.editor = editor;
    }
    self
  }

  /// Starts reading the accounts' plan limits outside runs (`orchester_workspace::plan_limits::reads`), until the
  /// session ends. Only the app's own window does: a test or a QA copy would read the person's real accounts.
  pub fn read_plan_limits(&mut self) {
    if let Some(running) = &mut self.running {
      let reads = running.services.plan_reads.clone();
      running.background.hold(running.services.runtime.spawn(reads.run()));
    }
  }

  /// Asks Orchester Cloud for a newer release now and every [`CHECK_EVERY`] until the session ends, downloading it in
  /// a copy that updates itself (`orchester_workspace::app_updates`). Only the app's own program does: a test's or a QA
  /// example's window never asks the cloud.
  pub fn check_for_updates(&mut self) {
    if let Some(running) = &mut self.running {
      let checks = app_updates::keep_checking(running.services.updates.clone(), CHECK_EVERY);
      running.background.hold(running.services.runtime.spawn(checks));
    }
  }

  /// Hands `link`, the `orchester://` link this program was started with, to the app that has the data folder open,
  /// when another has (`open_link::pipe`), and brings that app's window to the front: `true` once it took the link,
  /// and this program ends without a window. `false` leaves it to its *Orchester is already open* screen, or to its
  /// own session.
  pub fn hand_over_link(&self, runtime: &Handle, link: &str) -> bool {
    let Start::Problem(Problem {
      problem: StartupProblem::AlreadyOpen { folder, holder },
      ..
    }) = &self.start
    else {
      return false;
    };
    if let Err(error) = open_link::pipe::hand_over(runtime, folder, *holder, link) {
      tracing::warn!(%error, "the link was not handed to the app that has the data folder open");
      return false;
    }
    tracing::info!("a link was handed to the app that has the data folder open");
    if let Some(holder) = holder
      && super::front::bring_to_front(*holder) == super::front::Front::NoAnswer
    {
      tracing::info!("the system kept the window of the app that took the link where it was");
    }
    true
  }

  /// Opens the `orchester://` links of this session: `first`, the one this program was started with, once the window
  /// is up, and those other programs hand over until the session ends. Only the app's own program takes them: a test
  /// or a QA copy never makes the data folder's link pipe.
  pub fn take_links(&mut self, first: Option<String>) {
    let Some(running) = &mut self.running else {
      return;
    };
    if let Some(first) = first {
      running.services.opened_links.push(OpenLink::parse(&first));
    }
    let inbox = running.services.opened_links.clone();
    match open_link::pipe::listen(&running.services.runtime, &running.folder, inbox) {
      Ok(listening) => running.background.hold(listening),
      Err(error) => tracing::warn!(%error, "this window takes no links from other programs"),
    }
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
      ..
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
