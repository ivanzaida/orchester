//! The programs the desktop apps run for GitHub sync (`gh`) and git, shared with their screens. The
//! application runs real processes; tests hand in a runner that answers with
//! captured output, so screens that talk to GitHub can be driven without touching the real `gh`.

use std::{sync::Arc, time::Duration};

use orchester_core::command::CommandRunner;

use crate::ProcessRunner;

/// A `gh` that does not answer within this time counts as not running, as for connection checks.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// Git work that changes the repository (a merge with the repository's hooks, removing a worktree
/// full of build output) gets far longer: stopping git halfway would leave the repository half
/// changed.
const GIT_WORK_TIMEOUT: Duration = Duration::from_secs(60 * 60);

type Runner = Arc<dyn CommandRunner + Send + Sync>;

#[derive(Clone)]
pub struct Programs {
  quick: Runner,
  git_work: Runner,
  /// Whether these are real child processes, which [`Programs::timed`] stops after a time of its own.
  processes: bool,
}

impl Programs {
  /// Real child processes.
  pub fn processes() -> Self {
    Self {
      quick: Arc::new(ProcessRunner::new(COMMAND_TIMEOUT)),
      git_work: Arc::new(ProcessRunner::new(GIT_WORK_TIMEOUT)),
      processes: true,
    }
  }

  /// Programs run by `runner`, e.g. captured answers in tests.
  pub fn with_runner(runner: Runner) -> Self {
    Self {
      quick: runner.clone(),
      git_work: runner,
      processes: false,
    }
  }

  pub fn runner(&self) -> &(dyn CommandRunner + Send + Sync) {
    &*self.quick
  }

  /// The runner for git work that changes the repository: merging a run, removing its worktrees.
  pub fn git_work(&self) -> &(dyn CommandRunner + Send + Sync) {
    &*self.git_work
  }

  /// A runner for a program with a time of its own, e.g. a command tool's: real processes stopped after `timeout`,
  /// or the runner [`Programs::with_runner`] was given.
  pub fn timed(&self, timeout: Duration) -> Runner {
    if self.processes {
      Arc::new(ProcessRunner::new(timeout))
    } else {
      self.git_work.clone()
    }
  }
}

impl PartialEq for Programs {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.quick, &other.quick) && Arc::ptr_eq(&self.git_work, &other.git_work)
  }
}
