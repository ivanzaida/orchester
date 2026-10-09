//! The files of the worktree changed since the run's base revision, which the code index does not describe: git
//! lists them with the native agent's hardening ([`GIT_GLOBAL`]: no fsmonitor program, no pager, no optional locks, no
//! bare repository found from a working folder), pinned to the worktree and its own git folder (`--git-dir`,
//! `--work-tree`), so a `.git` file the agent rewrote cannot lead it to a configuration of the agent's making, with an
//! empty hooks folder the runner owns, and without entering a nested repository (`submodule.recurse=false`,
//! `diff.ignoreSubmodules=all`, `--ignore-submodules=all`), whose own configuration could name filter programs.

use std::{path::Path, time::Duration};

use super::{OutputKept, Revision, ToolContext, ToolProgram};
use crate::{command::Command, native::command::GIT_GLOBAL};

/// How long listing the changes may take.
const GIT_TIMEOUT: Duration = Duration::from_secs(60);
/// How much of git's output is kept: the start of the list.
const GIT_OUTPUT: usize = 56 * 1024;
const PROGRAM: &str = "git";
/// Settings that keep git out of nested repositories.
const NO_SUBMODULES: [&str; 3] = [
  "submodule.recurse=false",
  "diff.ignoreSubmodules=all",
  "status.submoduleSummary=false",
];

/// The programs that list the files of `context`'s worktree changed since revision `base` (`-z`, NUL
/// between paths): changed or deleted tracked files, then files git does not track and does not ignore.
pub fn changed_files(base: &Revision, context: &ToolContext<'_>) -> [ToolProgram; 2] {
  let diff = vec![
    "diff".to_owned(),
    "--ignore-submodules=all".to_owned(),
    "--name-only".to_owned(),
    "--no-renames".to_owned(),
    "-z".to_owned(),
    "--end-of-options".to_owned(),
    base.as_str().to_owned(),
    "--".to_owned(),
  ];
  let untracked = ["ls-files", "-z", "--others", "--exclude-standard"]
    .map(str::to_owned)
    .to_vec();
  [diff, untracked].map(|tail| hardened(tail, context))
}

/// Git with `rest` (the subcommand and its arguments) after the hardening every call gets, in `context`'s worktree.
fn hardened(rest: Vec<String>, context: &ToolContext<'_>) -> ToolProgram {
  let mut args: Vec<String> = GIT_GLOBAL.map(str::to_owned).to_vec();
  args.extend([
    format!("--git-dir={}", context.git_dir.display()),
    format!("--work-tree={}", context.worktree.display()),
    "-c".to_owned(),
    format!("core.hooksPath={}", context.hooks.display()),
  ]);
  for setting in NO_SUBMODULES {
    args.extend(["-c".to_owned(), setting.to_owned()]);
  }
  args.extend(rest);
  let mut variables = context.variables();
  variables
    .extend([("LC_ALL", "C"), ("GIT_TERMINAL_PROMPT", "0")].map(|(name, value)| (name.to_owned(), value.to_owned())));
  let command = Command::new(Path::new(PROGRAM), args)
    .with_current_dir(context.worktree)
    .with_environment(variables);
  ToolProgram::new(command, GIT_TIMEOUT, (GIT_OUTPUT, OutputKept::Head))
}
