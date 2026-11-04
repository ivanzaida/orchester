//! The Orchestrator chat's git in a run's worktree, which the runner runs for the desktop
//! (`docs/architecture/remote-runners.md`, rule 5): the desktop decides what to run, as for the person's checkout, and
//! sends each git command, or a question about files git named, as a [`RunGitRequest`]; the runner runs it in the
//! run's worktree, which it finds itself, and answers with a [`RunGitAnswer`]. A run's worktree belongs to its agents
//! until the run ended: the runner then takes only [`GitAccess::Read`] requests, and of those only commands that read
//! ([`only_reads`]).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
  command::{Command, CommandError, CommandOutput},
  diff::git::local_git_command,
  id::RunId,
};

/// The `git` program, looked up on the `PATH` as the runner does.
const GIT: &str = "git";

/// Whether a request only reads the run's worktree or belongs to a write, refused while the run has not ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitAccess {
  Read,
  Write,
}

/// Git work in the worktree of run `run`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunGitRequest {
  pub run: RunId,
  pub access: GitAccess,
  pub call: RunGitCall,
}

impl RunGitRequest {
  /// Whether it may run in the worktree of a run that has not ended: a read, of git only a command that reads.
  pub fn allowed_before_end(&self) -> bool {
    self.access == GitAccess::Read
      && match &self.call {
        RunGitCall::Git(git) => only_reads(&git.args),
        RunGitCall::Paths { .. } => true,
      }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunGitCall {
  /// One git command, as [`chat_git_command`] makes it.
  Git(GitInvocation),
  /// What is at `paths`, as git named them: relative to the worktree, or absolute (git's own folder).
  Paths { paths: Vec<String> },
}

/// `git <args>` with `input` on its standard input and `ssh` as its ssh command, stopped after `timeout_ms`; with
/// `cap`, at most that many bytes of each output are kept, read as git writes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInvocation {
  pub args: Vec<String>,
  pub input: Option<String>,
  pub ssh: Option<String>,
  pub timeout_ms: u64,
  pub cap: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunGitAnswer {
  /// git ran: its output, and whether some of it was left out.
  Ran { output: CommandOutput, cut: bool },
  /// git could not be run or did not finish.
  NotRun { error: CommandError },
  /// What is at each path asked about, in order.
  Paths { facts: Vec<PathFacts> },
}

/// What is at a path: whether anything is, and a file's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathFacts {
  pub exists: bool,
  /// The size of a file; `None` for a folder or nothing.
  pub file_len: Option<u64>,
}

/// `git <args>` in `dir` as the chat's git tools run it: git's messages in English and without the caller's
/// variables that would point git at another repository, index or settings; never waiting for a person — no terminal
/// prompt, no credential window, no editor; paths never pathspecs with magic; with `ssh` as its ssh command and `input`
/// on its standard input.
pub fn chat_git_command(dir: &Path, args: &[&str], input: Option<&str>, ssh: Option<&str>) -> Command {
  let settings = ["-c", "color.ui=never", "-c", "core.quotepath=false"];
  let command = local_git_command(Path::new(GIT), dir, settings.iter().chain(args).copied())
    // Nobody answers a prompt: git fails at once where it would ask for a user name, a password or an editor's text.
    .with_env("GIT_TERMINAL_PROMPT", "0")
    .with_env("GCM_INTERACTIVE", "never")
    .with_env("GIT_EDITOR", ":")
    .with_env("GIT_SEQUENCE_EDITOR", ":")
    .with_env("GIT_MERGE_AUTOEDIT", "no")
    // A path the chat names is a path, never a pathspec with magic (`:(top)`, globs).
    .with_env("GIT_LITERAL_PATHSPECS", "1");
  let command = match ssh {
    Some(ssh) => command.with_env("GIT_SSH_COMMAND", ssh),
    None => command,
  };
  match input {
    Some(text) => command.with_stdin(text.to_owned()),
    None => command,
  }
}

/// Git's commands that only read, whatever follows them.
const READS: [&str; 11] = [
  "status",
  "rev-parse",
  "log",
  "diff",
  "show",
  "blame",
  "for-each-ref",
  "check-ref-format",
  "ls-files",
  "ls-tree",
  "merge-base",
];

/// Whether `git <args>` only reads the repository: one of [`READS`], `remote` that only lists, `config --get`,
/// `symbolic-ref` that reads HEAD, or `hash-object` that writes no object.
pub fn only_reads(args: &[String]) -> bool {
  let Some((command, rest)) = args.split_first() else {
    return false;
  };
  let operands = || rest.iter().filter(|arg| !arg.starts_with('-')).count();
  match command.as_str() {
    read if READS.contains(&read) => true,
    "remote" => rest.iter().all(|arg| arg == "-v"),
    "config" => rest.first().is_some_and(|arg| arg == "--get"),
    "symbolic-ref" => operands() <= 1,
    "hash-object" => !rest.iter().any(|arg| arg == "-w"),
    _ => false,
  }
}
