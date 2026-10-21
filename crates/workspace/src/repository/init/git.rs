//! The git commands of [`super`]: each runs in one folder, with git's messages in English, without optional locks
//! and without the caller's variables that would point git at another repository, index or settings
//! (`orchester_core::diff::git::local_git_command`).

use std::path::Path;

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  diff::git::local_git_command,
};

/// The `git` program, looked up on the `PATH` like the runner does.
const GIT: &str = "git";

/// `git <args>` in `dir`.
pub(super) fn command<'a>(dir: &Path, args: impl IntoIterator<Item = &'a str>) -> Command {
  local_git_command(Path::new(GIT), dir, args)
}

/// What git wrote about a failure: its error output, or its output when it wrote its reason there.
pub(super) fn said(output: &CommandOutput) -> String {
  let error = output.stderr.trim();
  if error.is_empty() {
    output.stdout.trim().to_owned()
  } else {
    error.to_owned()
  }
}

/// The paths `output` lists, `-z` separated.
pub(super) fn listed(output: &CommandOutput) -> usize {
  output.stdout.split('\0').filter(|name| !name.is_empty()).count()
}

/// Whether the repository in `folder` has a commit at `HEAD`.
pub(super) fn has_commits(runner: &dyn CommandRunner, folder: &Path) -> Result<bool, CommandError> {
  let output = runner.run(&command(folder, ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]))?;
  match output.code {
    Some(0) => Ok(true),
    Some(1) => Ok(false),
    _ => Err(CommandError::Failed(said(&output))),
  }
}

/// The branch `HEAD` is on; `None` on a detached `HEAD` or when git does not say.
pub(super) fn current_branch(runner: &dyn CommandRunner, folder: &Path) -> Option<String> {
  let output = runner
    .run(&command(folder, ["symbolic-ref", "--quiet", "--short", "HEAD"]))
    .ok()?;
  let branch = output.stdout.trim();
  (output.success() && !branch.is_empty()).then(|| branch.to_owned())
}

/// The paths staged in the index of the repository in `folder`.
pub(super) fn staged(runner: &dyn CommandRunner, folder: &Path) -> Result<usize, CommandError> {
  let output = runner.run(&command(folder, ["ls-files", "-z", "--cached"]))?;
  if output.success() {
    Ok(listed(&output))
  } else {
    Err(CommandError::Failed(said(&output)))
  }
}
