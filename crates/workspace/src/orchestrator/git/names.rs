//! What the chat names to git — revisions, branches, tags, remotes — checked before git reads them, so that none is
//! read as one of git's options (`--output=…` writes a file) and a new branch or tag has a name git takes.

use super::command::Git;
use crate::app_mcp::ToolError;

/// `value`, a revision, range or remote's name the chat named for `argument`: not empty, not an option, one line.
pub(super) fn revision<'a>(argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let value = value.trim();
  if value.is_empty() || value.starts_with('-') || value.contains(['\n', '\r', '\0']) {
    return Err(ToolError::InvalidArguments(format!(
      "{argument}: {value:?} is not a revision git reads"
    )));
  }
  Ok(value)
}

/// `name`, a new branch's name git takes (`git check-ref-format --branch`).
pub(super) fn branch<'a>(git: &Git, name: &'a str) -> Result<&'a str, ToolError> {
  let name = revision("name", name)?;
  if git.succeeds(&["check-ref-format", "--branch", name])? {
    return Ok(name);
  }
  Err(ToolError::InvalidArguments(format!(
    "{name:?} is not a branch name git takes"
  )))
}

/// `name`, a new tag's name git takes (`git check-ref-format refs/tags/<name>`).
pub(super) fn tag<'a>(git: &Git, name: &'a str) -> Result<&'a str, ToolError> {
  let name = revision("name", name)?;
  if git.succeeds(&["check-ref-format", &format!("refs/tags/{name}")])? {
    return Ok(name);
  }
  Err(ToolError::InvalidArguments(format!(
    "{name:?} is not a tag name git takes"
  )))
}

/// The paths the chat named, each checked not to be read as an option; they always follow `--`.
pub(super) fn paths(paths: &[String]) -> Result<Vec<&str>, ToolError> {
  paths
    .iter()
    .map(|path| {
      if path.trim().is_empty() || path.contains('\0') {
        Err(ToolError::InvalidArguments(format!("{path:?} is not a path")))
      } else {
        Ok(path.as_str())
      }
    })
    .collect()
}

/// The branch HEAD is on; `None` on a detached HEAD.
pub(super) fn current_branch(git: &Git) -> Result<Option<String>, ToolError> {
  let output = git.run(&["symbolic-ref", "--quiet", "--short", "HEAD"], None)?;
  let branch = output.stdout.trim();
  Ok((output.success() && !branch.is_empty()).then(|| branch.to_owned()))
}

/// The remote `branch`'s upstream is on; `None` without one.
pub(super) fn upstream_remote(git: &Git, branch: &str) -> Result<Option<String>, ToolError> {
  let output = git.run(&["config", "--get", &format!("branch.{branch}.remote")], None)?;
  let remote = output.stdout.trim();
  Ok((output.success() && !remote.is_empty() && remote != ".").then(|| remote.to_owned()))
}

/// The commit `rev` names, in full; `None` when it names none.
pub(super) fn commit_of(git: &Git, rev: &str) -> Result<Option<String>, ToolError> {
  let output = git.run(
    &[
      "rev-parse",
      "--verify",
      "--quiet",
      "--end-of-options",
      &format!("{rev}^{{commit}}"),
    ],
    None,
  )?;
  let commit = output.stdout.trim();
  Ok((output.success() && !commit.is_empty()).then(|| commit.to_owned()))
}
