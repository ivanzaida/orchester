//! Where the chat's git runs (`docs/architecture/remote-runners.md`, rule 5): in the person's checkout the desktop
//! runs it; in a run's worktree the project's runner does ([`RunGit`]), the desktop only deciding what to run. Both
//! run git the same way (`orchester_exec::run_git::chat_git`), with the same times and the same cap on what a read
//! answers with. A run's worktree belongs to its agents until the run ended: the runner then refuses all but reads
//! (`run_active`), and a worktree gone since is refused as `not_a_repository`.

use std::{path::Path, time::Duration};

use orchester_core::{
  command::CommandOutput,
  id::RunId,
  orchestrator::run_git::{GitAccess, GitInvocation, PathFacts, RunGitAnswer, RunGitCall, RunGitRequest},
};
use orchester_exec::{
  programs::Programs,
  run_git::{chat_git, path_facts},
};
use orchester_ipc::Rejection;

use super::command::{not_run, refused, shown};
use crate::{app_mcp::ToolError, messages::Message, runner::RunRequestError};

/// The chat's git in the worktrees of a project's runs: what its runner answers ([`crate::runner::ThroughRunner`]).
pub trait RunGit: Send + Sync {
  /// Carries out `request` in its run's worktree. Blocks: called off the async workers.
  fn call(&self, request: RunGitRequest) -> Result<RunGitAnswer, RunRequestError>;
}

/// Where one tool's git runs.
#[derive(Clone, Copy)]
pub(super) enum Host<'a> {
  /// The person's checkout at `dir`, git run here with `programs`.
  Here { programs: &'a Programs, dir: &'a Path },
  /// The worktree of run `run` (number `number`), git run by its runner (`runs`), for a tool that does `access`.
  Run {
    runs: &'a dyn RunGit,
    run: RunId,
    number: u32,
    access: GitAccess,
  },
}

impl Host<'_> {
  /// Runs `git <args>` with `input` and `ssh`, stopped after `timeout`; with `cap`, at most that many bytes of each
  /// output kept. Its output and whether some was left out; git that cannot be run or did not finish is refused.
  pub(super) fn git(
    &self,
    (args, input, ssh): (&[&str], Option<&str>, Option<&str>),
    timeout: Duration,
    cap: Option<usize>,
  ) -> Result<(CommandOutput, bool), ToolError> {
    if let Self::Here { programs, dir } = *self {
      return chat_git(programs, dir, (args, input, ssh), (timeout, cap)).map_err(|error| not_run(&error, args));
    }
    let invocation = GitInvocation {
      args: args.iter().map(|arg| (*arg).to_owned()).collect(),
      input: input.map(str::to_owned),
      ssh: ssh.map(str::to_owned),
      timeout_ms: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
      cap: cap.map(|cap| u64::try_from(cap).unwrap_or(u64::MAX)),
    };
    match self.ask(RunGitCall::Git(invocation), args)? {
      RunGitAnswer::Ran { output, cut } => Ok((output, cut)),
      RunGitAnswer::NotRun { error } => Err(not_run(&error, args)),
      RunGitAnswer::Paths { .. } => Err(unexpected(args)),
    }
  }

  /// What is at each of `paths`, as git named them: relative to the folder, or absolute.
  pub(super) fn facts(&self, paths: &[String]) -> Result<Vec<PathFacts>, ToolError> {
    if let Self::Here { dir, .. } = *self {
      return Ok(paths.iter().map(|path| path_facts(&dir.join(path))).collect());
    }
    let call = RunGitCall::Paths { paths: paths.to_vec() };
    match self.ask(call, &[])? {
      RunGitAnswer::Paths { facts } if facts.len() == paths.len() => Ok(facts),
      _ => Err(unexpected(&[])),
    }
  }

  /// `call` asked of the run's runner; what it refused told as the tools tell it (`args`, the git it is about).
  fn ask(&self, call: RunGitCall, args: &[&str]) -> Result<RunGitAnswer, ToolError> {
    let Self::Run {
      runs,
      run,
      number,
      access,
    } = *self
    else {
      return Err(unexpected(args));
    };
    let number = number.to_string();
    runs
      .call(RunGitRequest { run, access, call })
      .map_err(|error| match error {
        RunRequestError::Rejected(Rejection::RunNotEnded) => refused(
          "run_active",
          Message::new("orchestrator.git.runActive").with("run", number),
        ),
        RunRequestError::Rejected(Rejection::WorktreeRemoved | Rejection::RunNotFound) => refused(
          "not_a_repository",
          Message::new("orchestrator.git.worktreeGone").with("run", number),
        ),
        other => failed(args, &other.to_string()),
      })
  }
}

/// The refusal of `git <args>` that did not run as asked, with why.
fn failed(args: &[&str], said: &str) -> ToolError {
  refused(
    "git_failed",
    Message::new("orchestrator.git.gitFailed")
      .with("command", shown(args))
      .with("said", said.to_owned()),
  )
}

/// The runner answered with something else than what was asked.
fn unexpected(args: &[&str]) -> ToolError {
  failed(args, "the runner's answer does not fit the request")
}
