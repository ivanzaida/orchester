//! The git command of each git tool. Every value of a call is one argument of its own, after the options
//! and an end marker (`--end-of-options` before a revision, `--` before paths), and git reads paths
//! literally (`--literal-pathspecs`), so no value of the agent's becomes an option, a pathspec pattern or a
//! second command. A commit message goes to standard input (`-F -`), not into the arguments.
//!
//! Git gets the native agent's hardening ([`GIT_GLOBAL`]: no fsmonitor program, no pager, no optional
//! locks, no bare repository found from a working folder) and, for diffs, no configured diff or textconv
//! program. It is pinned to the worktree and its own git folder (`--git-dir`, `--work-tree`), so a `.git`
//! file the agent rewrote cannot lead it to a configuration of the agent's making, with filter, signing or
//! hook programs; the repository's own configuration applies, as to the runner's checkpoint commits (a
//! `git lfs` filter keeps working). No hook runs: hooks are programs, which a call must not start, so the
//! hooks folder is an empty folder the runner owns (an absolute path, never one relative to the worktree)
//! and `--no-verify` is given too. Nothing is signed or checked against a signature (`commit.gpgSign`,
//! `log.showSignature`), which would start a signing program.
//!
//! Git never enters a nested repository: a submodule, or a repository the agent made in the worktree and
//! staged as a gitlink, keeps a configuration of its own, whose filter programs git would run when it looks
//! at that repository's files. Recursion is off for every call (`submodule.recurse=false`,
//! `diff.ignoreSubmodules=all`, `status.submoduleSummary=false`, and `--ignore-submodules=all` for the
//! commands that take it), so a gitlink is only a commit id to git, and `git_add` stages named files only,
//! none in a nested repository ([`refuse_nested`]).

use std::{
  path::{Component, Path},
  time::Duration,
};

use super::{GitCall, OutputKept, ToolCallError, ToolContext, ToolProgram, link::is_link};
use crate::{
  command::Command,
  native::command::{GIT_DIFF_OPTIONS, GIT_GLOBAL},
};

/// How long a git call may run: these read or write the worktree's own files and index.
pub const GIT_TIMEOUT: Duration = Duration::from_secs(60);
/// How much of git's output the agent gets: the start, where a diff or log begins.
const GIT_OUTPUT: usize = 128 * 1024;
const PROGRAM: &str = "git";
/// Settings that keep git out of nested repositories.
const NO_SUBMODULES: [&str; 3] = [
  "submodule.recurse=false",
  "diff.ignoreSubmodules=all",
  "status.submoduleSummary=false",
];
const IGNORE_SUBMODULES: &str = "--ignore-submodules=all";
/// The entry that makes a folder a repository of its own.
const GIT_ENTRY: &str = ".git";

/// Refuses a `git_add` of a folder, or of a path in `context`'s worktree that is a nested repository or lies
/// in one. Adding a folder stages every nested repository below it as a gitlink, and a later `git add` of a
/// folder looks into each staged gitlink with its own configuration, filters included, whatever the
/// settings say (git 2.46, checked 2026-10-05); adding named files never does. So only files are staged,
/// listed one by one, a deleted one included.
pub(super) fn refuse_nested(call: &GitCall, context: &ToolContext<'_>) -> Result<(), ToolCallError> {
  let GitCall::Add { paths } = call else {
    return Ok(());
  };
  paths
    .iter()
    .try_for_each(|path| stageable(path.as_str(), context.worktree))
}

/// Refuses `path` unless every folder from `worktree` down to it is a plain folder of the worktree's own
/// repository and it is a plain file, or no longer exists (a deletion is staged too). A link or junction
/// anywhere on the way is refused: git would follow it into a folder of its choosing.
fn stageable(path: &str, worktree: &Path) -> Result<(), ToolCallError> {
  let refused = |error: fn(String) -> ToolCallError| Err(error(path.to_owned()));
  let parts: Vec<Component<'_>> = Path::new(path)
    .components()
    .filter(|part| matches!(part, Component::Normal(_)))
    .collect();
  let Some(last) = parts.len().checked_sub(1) else {
    return refused(ToolCallError::Folder);
  };
  let mut current = worktree.to_path_buf();
  for (at, part) in parts.iter().enumerate() {
    current.push(part);
    let Ok(entry) = current.symlink_metadata() else {
      return Ok(());
    };
    if is_link(&entry) {
      return refused(ToolCallError::Link);
    }
    if at < last && current.join(GIT_ENTRY).symlink_metadata().is_ok() {
      return refused(ToolCallError::NestedRepository);
    }
    if at == last && !entry.is_file() {
      return refused(ToolCallError::Folder);
    }
  }
  Ok(())
}

/// The git program of `call` in `context`'s worktree.
pub(super) fn program(call: &GitCall, context: &ToolContext<'_>) -> ToolProgram {
  let mut args: Vec<String> = GIT_GLOBAL.map(str::to_owned).to_vec();
  args.extend([
    format!("--git-dir={}", context.git_dir.display()),
    format!("--work-tree={}", context.worktree.display()),
    "-c".to_owned(),
    format!("core.hooksPath={}", context.hooks.display()),
    "-c".to_owned(),
    "log.showSignature=false".to_owned(),
  ]);
  for setting in NO_SUBMODULES {
    args.extend(["-c".to_owned(), setting.to_owned()]);
  }
  let mut stdin = None;
  let tail: Vec<String> = match call {
    GitCall::Status => words(&["status", "--short", "--branch", IGNORE_SUBMODULES]),
    GitCall::Diff { path, staged } => {
      args.push("--literal-pathspecs".to_owned());
      let mut tail = words(&["diff", IGNORE_SUBMODULES]);
      tail.extend(words(&GIT_DIFF_OPTIONS));
      if *staged {
        tail.push("--cached".to_owned());
      }
      tail.push("--".to_owned());
      tail.extend(path.iter().map(|path| path.as_str().to_owned()));
      tail
    }
    GitCall::Log { path, count } => {
      args.push("--literal-pathspecs".to_owned());
      let mut tail = words(&["log", IGNORE_SUBMODULES]);
      tail.extend(words(&GIT_DIFF_OPTIONS));
      tail.push(format!("--max-count={count}"));
      tail.push("--".to_owned());
      tail.extend(path.iter().map(|path| path.as_str().to_owned()));
      tail
    }
    GitCall::Show { revision, path } => {
      let mut tail = words(&["show", IGNORE_SUBMODULES]);
      tail.extend(words(&GIT_DIFF_OPTIONS));
      tail.push("--end-of-options".to_owned());
      tail.push(match path {
        // A tree path is written with `/` on every platform.
        Some(path) => format!("{}:{}", revision.as_str(), path.as_str().replace('\\', "/")),
        None => revision.as_str().to_owned(),
      });
      tail
    }
    GitCall::Add { paths } => {
      args.push("--literal-pathspecs".to_owned());
      let mut tail = words(&["add", "--"]);
      tail.extend(paths.iter().map(|path| path.as_str().to_owned()));
      tail
    }
    GitCall::Commit { message } => {
      args.extend(words(&["-c", "commit.gpgSign=false"]));
      stdin = Some(message.as_str().to_owned());
      words(&["commit", "--no-verify", "-F", "-"])
    }
  };
  args.extend(tail);
  let mut variables = context.variables();
  variables
    .extend([("LC_ALL", "C"), ("GIT_TERMINAL_PROMPT", "0")].map(|(name, value)| (name.to_owned(), value.to_owned())));
  let command = Command::new(Path::new(PROGRAM), args)
    .with_current_dir(context.worktree)
    .with_environment(variables);
  let command = match stdin {
    Some(message) => command.with_stdin(message),
    None => command,
  };
  ToolProgram::new(command, GIT_TIMEOUT, (GIT_OUTPUT, OutputKept::Head))
}

fn words(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}
