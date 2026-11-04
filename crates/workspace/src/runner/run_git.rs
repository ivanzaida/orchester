//! The Orchestrator chat's git in a run's worktree, which the project's runner runs ([`Command::RunGit`],
//! `docs/architecture/remote-runners.md` rule 5): the desktop never runs git there itself.

use std::path::PathBuf;

use orchester_core::orchestrator::run_git::{RunGitAnswer, RunGitRequest};
use orchester_ipc::{Command, Reply};
use orchester_store::ProjectDb;

use super::{RequestError, RunRequestError, Runners};
use crate::{orchestrator::git::RunGit, workspace::OpenProject};

impl Runners {
  /// Asks `project`'s runner, starting it if needed, to carry out `request` in its run's worktree.
  pub async fn run_git_in(
    &self,
    project: &OpenProject,
    request: RunGitRequest,
  ) -> Result<RunGitAnswer, RunRequestError> {
    match self.ask_project(project, Command::RunGit(request)).await? {
      Reply::RunGit(answer) => Ok(answer),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to RunGit");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// The chat's git in the runs' worktrees of the project at `root`, through its runner.
  pub fn run_git(&self, root: PathBuf) -> ThroughRunner {
    ThroughRunner {
      runners: self.clone(),
      root,
    }
  }
}

/// The chat's git in the runs' worktrees of the project at `root`, which its runner runs.
#[derive(Clone)]
pub struct ThroughRunner {
  runners: Runners,
  root: PathBuf,
}

impl RunGit for ThroughRunner {
  /// Blocks on SQLite and on the runner's answer: called off the async workers.
  fn call(&self, request: RunGitRequest) -> Result<RunGitAnswer, RunRequestError> {
    let project = ProjectDb::open(&self.root)
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project for the chat's git");
        RunRequestError::Request(RequestError::NotConnected)
      })?
      .project()
      .clone();
    let project = OpenProject {
      project,
      repo_root: self.root.clone(),
    };
    self
      .runners
      .inner
      .runtime
      .block_on(self.runners.run_git_in(&project, request))
  }
}
