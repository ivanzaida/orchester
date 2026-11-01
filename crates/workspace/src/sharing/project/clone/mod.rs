//! Cloning a shared project's repository from *Link a folder* (ORC-584, `docs/design/startup.md`, "Clone in a
//! console"): Git clones `https://<hint>.git` in a console window the person sees, where it may ask them to sign in
//! with the credentials Git already uses (`gh` is not needed: Git asks its own credential helper, which may be `gh`).
//! The clone is used only when Git ended with 0 **and** the new folder is a checkout whose `origin` is the project's
//! hint ([`check_folder`]); nothing is linked here, and nothing is ever deleted: a destination that already holds
//! anything is refused before a console opens, and what a failed clone left is named and kept.
//!
//! [`Clones`] keeps each project's clone while it runs and after it ended, so a page left and opened again follows
//! the same console instead of opening another.

mod destination;
mod system;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use orchester_api::SharedProject;
use orchester_core::id::ServerId;
use orchester_exec::{
  clone_console::{ConsoleEnd, ConsoleWords},
  programs::Programs,
};
use parking_lot::Mutex;
use tokio::sync::watch;

pub use self::{
  destination::{Destination, check_destination, plain_destination},
  system::find_git,
};
use super::{
  folder::{FolderCheck, check_folder},
  landing::clone_url,
};
use crate::background;

/// A clone the person asked for.
#[derive(Clone, Debug)]
pub struct CloneRequest {
  pub server: ServerId,
  pub project: SharedProject,
  /// The folder for the checkout: one that doesn't exist yet, or an empty one.
  pub destination: PathBuf,
  /// What the console window says.
  pub words: ConsoleWords,
}

/// Which project's clone: its server and slug.
pub type CloneKey = (ServerId, String);

/// What the console is to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchJob {
  pub url: String,
  pub destination: PathBuf,
  pub words: ConsoleWords,
}

/// How a clone that ran ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloneEnd {
  /// Git finished and `folder` is a checkout of the project: it can be linked.
  Cloned { folder: PathBuf },
  /// Git said it finished, but `folder` is not a checkout of the project, as `check` says.
  NotCheckout { folder: PathBuf, check: FolderCheck },
  /// Git stopped with `code`, a sign-in refused among the reasons; its message is in the console. `left` is the
  /// folder when Git left something in it.
  Failed { code: i32, left: Option<PathBuf> },
  /// The console closed, or the clone was interrupted there, before Git ended.
  Closed { left: Option<PathBuf> },
}

/// Why no clone ran, or how one was lost.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CloneRefusal {
  /// The project names no repository, or one an address can't hold safely.
  #[error("the project names no repository to clone")]
  NoUrl,
  /// The destination is not a place for a new checkout; nothing opened.
  #[error("the folder can't take a clone")]
  Destination(Destination),
  /// No git is installed that can run the clone.
  #[error("git is not installed")]
  GitMissing,
  #[error("the console window did not open")]
  NotOpened,
  /// Terminal opened but never ran the clone (macOS).
  #[error("the terminal did not start the clone")]
  NotStarted,
  #[error("cloning in a console is not supported on this platform yet")]
  Unsupported,
  /// The console ran, but how it ended can't be told; the cause is logged.
  #[error("the console's end could not be read")]
  Lost,
  /// Work on a background thread ended before it finished; the cause is logged.
  #[error("the work was interrupted")]
  Interrupted,
}

/// Where a clone is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloneState {
  Running,
  Ended(Result<CloneEnd, CloneRefusal>),
}

/// Runs a clone's console to its end: blocking, for as long as the person keeps the console open.
pub type Launcher = Arc<dyn Fn(&LaunchJob) -> Result<ConsoleEnd, CloneRefusal> + Send + Sync>;

/// The session's clones, by project. Clones share them.
#[derive(Clone)]
pub struct Clones {
  launcher: Launcher,
  clones: Arc<Mutex<HashMap<CloneKey, watch::Receiver<CloneState>>>>,
}

impl Clones {
  /// Clones whose consoles `launch` runs: the system's ([`Clones::system`]), or a stand-in in a test or a QA copy.
  pub fn with_launcher(
    launch: impl Fn(&LaunchJob) -> Result<ConsoleEnd, CloneRefusal> + Send + Sync + 'static,
  ) -> Self {
    Self {
      launcher: Arc::new(launch),
      clones: Arc::default(),
    }
  }

  /// The clone of `key` running now, or the last one that ended.
  pub fn follow(&self, key: &CloneKey) -> Option<CloneState> {
    self.clones.lock().get(key).map(|state| state.borrow().clone())
  }

  /// The end of `key`'s clone: at once for one that ended, once it ends for one that runs; `None` when it had none.
  pub async fn ended(&self, key: &CloneKey) -> Option<Result<CloneEnd, CloneRefusal>> {
    let state = self.clones.lock().get(key).cloned()?;
    Some(ended(state).await)
  }

  /// Clones as `request` asks and waits for its end; `programs` checks the new folder. While the project's clone
  /// runs, this follows it instead of opening another console. The address and the destination are checked again
  /// first: an occupied destination is refused before anything opens. The clone goes on when the caller stops
  /// waiting, and [`Self::follow`] still finds it.
  pub async fn start(&self, programs: Programs, request: CloneRequest) -> Result<CloneEnd, CloneRefusal> {
    let CloneRequest {
      server,
      project,
      destination,
      words,
    } = request;
    let key = (server, project.slug.clone());
    if let Some(running) = self.running(&key) {
      return ended(running).await;
    }
    let url = project
      .remote_hint
      .as_ref()
      .and_then(clone_url)
      .ok_or(CloneRefusal::NoUrl)?;
    let destination = plain_destination(&destination);
    let place = destination.clone();
    let checked = background::run(move || check_destination(&place))
      .await
      .map_err(|_| CloneRefusal::Interrupted)?;
    if !checked.allows() {
      return Err(CloneRefusal::Destination(checked));
    }
    let job = LaunchJob {
      url,
      destination,
      words,
    };
    let receiver = {
      let mut clones = self.clones.lock();
      match clones.get(&key).filter(|state| *state.borrow() == CloneState::Running) {
        Some(running) => running.clone(),
        None => {
          let (sender, receiver) = watch::channel(CloneState::Running);
          clones.insert(key, receiver.clone());
          let run = (self.launcher.clone(), programs, server, project);
          tokio::spawn(async move { sender.send_replace(CloneState::Ended(cloned(run, job).await)) });
          receiver
        }
      }
    };
    ended(receiver).await
  }

  /// The project's clone while it runs.
  fn running(&self, key: &CloneKey) -> Option<watch::Receiver<CloneState>> {
    self
      .clones
      .lock()
      .get(key)
      .filter(|state| *state.borrow() == CloneState::Running)
      .cloned()
  }
}

/// The end the clone `state` follows comes to.
async fn ended(mut state: watch::Receiver<CloneState>) -> Result<CloneEnd, CloneRefusal> {
  match state.wait_for(|state| *state != CloneState::Running).await {
    Ok(state) => match &*state {
      CloneState::Ended(end) => end.clone(),
      CloneState::Running => Err(CloneRefusal::Interrupted),
    },
    // The clone's task ended without saying how: it panicked, or the runtime stopped.
    Err(_) => Err(CloneRefusal::Interrupted),
  }
}

/// Runs `job`'s console and checks what it left.
async fn cloned(
  (launcher, programs, server, project): (Launcher, Programs, ServerId, SharedProject),
  job: LaunchJob,
) -> Result<CloneEnd, CloneRefusal> {
  let folder = job.destination.clone();
  let end = background::run(move || launcher(&job))
    .await
    .map_err(|_| CloneRefusal::Interrupted)??;
  background::run(move || outcome(&programs, (server, &project), folder, end))
    .await
    .map_err(|_| CloneRefusal::Interrupted)
}

/// What the clone into `folder` that ended as `end` gave. Blocking.
fn outcome(
  programs: &Programs,
  (server, project): (ServerId, &SharedProject),
  folder: PathBuf,
  end: ConsoleEnd,
) -> CloneEnd {
  match end {
    ConsoleEnd::Exited { code: 0 } => match check_folder(programs, (server, project), &folder) {
      Ok(FolderCheck::Matches) => CloneEnd::Cloned { folder },
      Ok(check) => CloneEnd::NotCheckout { folder, check },
      Err(error) => {
        tracing::warn!(%error, "a new clone's project database could not be read");
        CloneEnd::NotCheckout {
          folder,
          check: FolderCheck::Unreadable,
        }
      }
    },
    ConsoleEnd::Exited { code } => CloneEnd::Failed {
      code,
      left: left(folder),
    },
    ConsoleEnd::Closed => CloneEnd::Closed { left: left(folder) },
  }
}

/// `folder` when the clone left something in it: it held nothing when the clone began.
fn left(folder: PathBuf) -> Option<PathBuf> {
  (check_destination(&folder) == Destination::Occupied).then_some(folder)
}
