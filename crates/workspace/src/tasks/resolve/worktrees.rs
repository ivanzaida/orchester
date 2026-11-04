//! A project's run worktrees as resolving by hand reaches them: through the project's runner, which runs git where the
//! worktree is (`docs/architecture/remote-runners.md`, rule 5); the desktop runs none there itself. A test whose
//! session has no runner has the same requests carried out in its own process, through the messages as the
//! connection serializes them ([`Runners::serve_worktrees_here_for_test`]).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  id::RunId,
  integration::resolve::{
    MergedIn, Resolution, Side, Started,
    request::{ResolveAnswer, ResolveFailure, ResolveRequest},
  },
  revision::Revision,
  run::BranchName,
};
#[cfg(feature = "test-support")]
use orchester_exec::programs::Programs;
use thiserror::Error;

use crate::runner::{RequestError, RunRequestError, Runners};

/// The worktrees of one project's runs, for resolving merges by hand in them.
#[derive(Clone)]
pub struct RunWorktrees {
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

/// Why a request about a run's worktree did not go on.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum Unresolved {
  /// The project's runner did not carry it out.
  #[error(transparent)]
  Runner(#[from] RunRequestError),
  /// It was carried out and refused, or git failed.
  #[error(transparent)]
  Failed(#[from] ResolveFailure),
}

impl RunWorktrees {
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

  /// Whether `run`'s worktree is there and, when it is, the merge in progress in it.
  pub(crate) fn read(&self, run: RunId) -> Result<(bool, Option<Resolution>), Unresolved> {
    match self.ask(run, ResolveRequest::Read)? {
      ResolveAnswer::Read { present, merge } => Ok((present, merge)),
      other => Err(unexpected(&other)),
    }
  }

  /// Whether `run`'s worktree is there.
  pub(crate) fn present(&self, run: RunId) -> Result<bool, Unresolved> {
    match self.ask(run, ResolveRequest::Present)? {
      ResolveAnswer::Present(present) => Ok(present),
      other => Err(unexpected(&other)),
    }
  }

  /// Whether a merge is in progress in `run`'s worktree.
  pub(crate) fn in_merge(&self, run: RunId) -> Result<bool, Unresolved> {
    match self.ask(run, ResolveRequest::InMerge)? {
      ResolveAnswer::InMerge(merging) => Ok(merging),
      other => Err(unexpected(&other)),
    }
  }

  /// The commit `HEAD` is at in `run`'s worktree.
  pub(crate) fn head(&self, run: RunId) -> Result<Revision, Unresolved> {
    match self.ask(run, ResolveRequest::Head)? {
      ResolveAnswer::Head(head) => Ok(head),
      other => Err(unexpected(&other)),
    }
  }

  /// Merges `base` into `branch`, checked out in `run`'s worktree at `expected`.
  pub(crate) fn start(
    &self,
    run: RunId,
    (base, branch, expected): (&BranchName, &BranchName, &Revision),
  ) -> Result<Started, Unresolved> {
    let request = ResolveRequest::Start {
      base: base.clone(),
      branch: branch.clone(),
      expected: expected.clone(),
    };
    match self.ask(run, request)? {
      ResolveAnswer::Started(started) => Ok(started),
      other => Err(unexpected(&other)),
    }
  }

  /// Keeps `side`'s version of `path` in `run`'s worktree.
  pub(crate) fn decide(&self, run: RunId, (path, side): (&str, Side)) -> Result<(), Unresolved> {
    let request = ResolveRequest::Decide {
      path: path.to_owned(),
      side,
    };
    self.done(run, request)
  }

  /// Commits the merge of `base` in progress in `run`'s worktree, its branch still at `expected`.
  pub(crate) fn finish(&self, run: RunId, (base, expected): (&BranchName, &Revision)) -> Result<MergedIn, Unresolved> {
    let request = ResolveRequest::Finish {
      base: base.clone(),
      expected: expected.clone(),
    };
    match self.ask(run, request)? {
      ResolveAnswer::Finished(merged) => Ok(merged),
      other => Err(unexpected(&other)),
    }
  }

  /// Aborts the merge in progress in `run`'s worktree.
  pub(crate) fn abort(&self, run: RunId) -> Result<(), Unresolved> {
    self.done(run, ResolveRequest::Abort)
  }

  /// Which of `files` in `run`'s worktree still hold a conflict marker.
  pub(crate) fn marked(&self, run: RunId, files: &[String]) -> Result<Vec<String>, Unresolved> {
    let request = ResolveRequest::Marked { files: files.to_vec() };
    match self.ask(run, request)? {
      ResolveAnswer::Marked(marked) => Ok(marked),
      other => Err(unexpected(&other)),
    }
  }

  fn done(&self, run: RunId, request: ResolveRequest) -> Result<(), Unresolved> {
    match self.ask(run, request)? {
      ResolveAnswer::Done => Ok(()),
      other => Err(unexpected(&other)),
    }
  }

  /// Carries out `request` in `run`'s worktree. Blocks on the runner and git: call it off the runtime's async workers.
  fn ask(&self, run: RunId, request: ResolveRequest) -> Result<ResolveAnswer, Unresolved> {
    match &self.reach {
      Reach::Runner { runners, name } => Ok(runners.resolve_blocking((&self.root, name), run, request)??),
      #[cfg(feature = "test-support")]
      Reach::Here(programs) => here::serve(&self.root, programs, run, &request),
    }
  }
}

/// An answer that does not answer the request: the runner and the desktop disagree, so what it did is not known, as
/// for a lost reply.
fn unexpected(answer: &ResolveAnswer) -> Unresolved {
  tracing::error!(?answer, "unexpected answer to a request about a run's worktree");
  RunRequestError::Request(RequestError::Lost).into()
}

/// A test's stand-in for the runner: the request and the reply go through the protocol's serialization.
#[cfg(feature = "test-support")]
mod here {
  use std::path::Path;

  use orchester_core::{
    id::RunId,
    integration::resolve::request::{self, ResolveAnswer, ResolveRequest},
  };
  use orchester_exec::programs::Programs;
  use orchester_ipc::{Command, Reply};
  use serde::{Serialize, de::DeserializeOwned};

  use super::Unresolved;
  use crate::{
    runner::{RequestError, RunRequestError},
    tasks::changes::{GIT, run_worktree},
  };

  pub(super) fn serve(
    root: &Path,
    programs: &Programs,
    run: RunId,
    request: &ResolveRequest,
  ) -> Result<ResolveAnswer, Unresolved> {
    let sent = Command::Resolve {
      run,
      request: request.clone(),
    };
    let Command::Resolve { run, request } = through_the_wire(&sent) else {
      return Err(lost());
    };
    let served = request::serve(
      (&programs.runner(), &programs.git_work()),
      Path::new(GIT),
      &run_worktree(root, run),
      &request,
    );
    let reply = match served {
      Ok(answer) => Reply::Resolved(answer),
      Err(failure) => Reply::ResolveFailed(failure),
    };
    match through_the_wire(&reply) {
      Reply::Resolved(answer) => Ok(answer),
      Reply::ResolveFailed(failure) => Err(failure.into()),
      _ => Err(lost()),
    }
  }

  fn lost() -> Unresolved {
    RunRequestError::Request(RequestError::Lost).into()
  }

  /// `value` as the other side of the connection reads it.
  fn through_the_wire<T: Serialize + DeserializeOwned>(value: &T) -> T {
    let text = serde_json::to_string(value).expect("a message of the protocol serializes");
    serde_json::from_str(&text).expect("a message of the protocol reads back as it was written")
  }
}
