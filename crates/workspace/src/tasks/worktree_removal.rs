//! A project's run worktrees as removing them reaches them: through the project's runner, which removes them, stops
//! the run's processes in a folder something held, prunes the codebase indexes and sweeps the worktree folders where
//! they are (`docs/architecture/remote-runners.md`, rule 5); the desktop does none of it itself. It keeps the
//! project's records of the removal (`run_worktree_removals`, `run_worktrees_not_removed`). A test whose session has
//! no runner has the same requests carried out in its own process, through the messages as the connection serializes
//! them ([`Runners::serve_worktrees_here_for_test`]).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  id::RunId,
  integration::{
    Cleanup,
    removal::{RemovalFailure, RemovalRequest},
  },
};
#[cfg(feature = "test-support")]
use orchester_exec::programs::Programs;
use thiserror::Error;

use crate::runner::{RunRequestError, Runners};

/// The worktrees of one project's runs, for removing what runs left.
#[derive(Clone)]
pub struct WorktreeRemover {
  root: PathBuf,
  reach: Reach,
}

#[derive(Clone)]
enum Reach {
  /// The project's runner, by the project's name, which starts it when it is not running.
  Runner { runners: Runners, name: Arc<str> },
  /// This process, with these programs: a test without a runner.
  #[cfg(feature = "test-support")]
  Here(Programs),
}

/// Why a removal removed nothing.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum Unremoved {
  /// The project's runner did not carry it out.
  #[error(transparent)]
  Runner(#[from] RunRequestError),
  /// It was carried out and refused, or git failed before it removed anything.
  #[error(transparent)]
  Failed(#[from] RemovalFailure),
}

impl WorktreeRemover {
  /// The worktrees of the runs of the project at `root`, named `name`, reached through its runner in `runners`.
  pub fn through_runner(runners: &Runners, (root, name): (&Path, &str)) -> Self {
    #[cfg(feature = "test-support")]
    if let Some(programs) = runners.worktrees_here() {
      return Self::here_for_test(root, programs);
    }
    Self {
      root: root.to_path_buf(),
      reach: Reach::Runner {
        runners: runners.clone(),
        name: name.into(),
      },
    }
  }

  /// The worktrees of the runs of the project at `root`, reached in this process with `programs`: for a test without
  /// a runner.
  #[cfg(feature = "test-support")]
  pub fn here_for_test(root: &Path, programs: Programs) -> Self {
    Self {
      root: root.to_path_buf(),
      reach: Reach::Here(programs),
    }
  }

  /// The repository of the project whose runs' worktrees these are.
  pub fn root(&self) -> &Path {
    &self.root
  }

  /// Removes what `run`, which ended and whose removal the caller recorded, left, as `request` allows: what it removed,
  /// and what stays and why. Blocks on the runner and git: call it off the runtime's async workers.
  pub(crate) fn remove(&self, run: RunId, request: RemovalRequest) -> Result<Cleanup, Unremoved> {
    match &self.reach {
      Reach::Runner { runners, name } => Ok(runners.remove_worktrees_blocking((&self.root, name), run, request)??),
      #[cfg(feature = "test-support")]
      Reach::Here(programs) => here::serve(&self.root, programs, run, request),
    }
  }
}

/// A test's stand-in for the runner: the request and the reply go through the protocol's serialization, and what the
/// runner does after a removal happens here too: the codebase indexes pruned, and the worktree folders swept on a
/// thread of their own.
#[cfg(feature = "test-support")]
mod here {
  use std::path::Path;

  use orchester_core::{
    id::RunId,
    integration::{
      Cleanup,
      removal::{self, RemovalFailure, RemovalRequest},
    },
    store::RunStore,
  };
  use orchester_exec::{programs::Programs, run_processes::RunProcesses};
  use orchester_index::CodebaseIndexes;
  use orchester_ipc::{Command, Reply};
  use orchester_store::ProjectDb;
  use serde::{Serialize, de::DeserializeOwned};

  use super::Unremoved;
  use crate::{
    runner::{RequestError, RunRequestError},
    tasks::changes::GIT,
  };

  pub(super) fn serve(
    root: &Path,
    programs: &Programs,
    run: RunId,
    request: RemovalRequest,
  ) -> Result<Cleanup, Unremoved> {
    let Command::RemoveWorktrees { run, request } = through_the_wire(&Command::RemoveWorktrees { run, request }) else {
      return Err(lost());
    };
    let reply = match carry_out(root, programs, run, &request) {
      Ok(cleanup) => Reply::WorktreesRemoved(cleanup),
      Err(failure) => Reply::RemovalFailed(failure),
    };
    match through_the_wire(&reply) {
      Reply::WorktreesRemoved(cleanup) => Ok(cleanup),
      Reply::RemovalFailed(failure) => Err(failure.into()),
      _ => Err(lost()),
    }
  }

  fn carry_out(
    root: &Path,
    programs: &Programs,
    run: RunId,
    request: &RemovalRequest,
  ) -> Result<Cleanup, RemovalFailure> {
    let unreadable = |error: &dyn std::error::Error| RemovalFailure::RunUnreadable(error.to_string());
    let run = ProjectDb::open_read_only(root)
      .map_err(|error| unreadable(&error))?
      .run(run)
      .map_err(|error| unreadable(&error))?
      .ok_or_else(|| RemovalFailure::RunUnreadable("the project has no such run".to_owned()))?;
    let stop = |folder: &Path| RunProcesses::stop_in(folder, run.created_at());
    let cleanup = removal::serve(&programs.git_work(), (Path::new(GIT), root), &run, request, stop)?;
    if let Err(error) = CodebaseIndexes::new(root).prune() {
      tracing::warn!(%error, "cannot remove the codebase indexes of removed worktrees");
    }
    sweep_in_background(programs, root);
    Ok(cleanup)
  }

  /// Sweeps the worktree folders on a thread of its own, as the runner does: a removal does not wait for other runs'
  /// folders, nor holds the checkout's integration lease while they go.
  fn sweep_in_background(programs: &Programs, root: &Path) {
    let (programs, root) = (programs.clone(), root.to_path_buf());
    let started = std::thread::Builder::new()
      .name("worktree-sweep".into())
      .spawn(move || match ProjectDb::open(&root) {
        Ok(db) => db.sweep_worktree_folders(&programs.git_work(), (Path::new(GIT), &root)),
        Err(error) => tracing::warn!(%error, "cannot read which runs own the worktree folders; none is swept"),
      });
    if let Err(error) = started {
      tracing::warn!(%error, "cannot start the sweep of worktree folders");
    }
  }

  fn lost() -> Unremoved {
    RunRequestError::Request(RequestError::Lost).into()
  }

  /// `value` as the other side of the connection reads it.
  fn through_the_wire<T: Serialize + DeserializeOwned>(value: &T) -> T {
    let text = serde_json::to_string(value).expect("a message of the protocol serializes");
    serde_json::from_str(&text).expect("a message of the protocol reads back as it was written")
  }
}
