//! What the chat names to git — revisions, branches, tags, remotes, paths — checked before git reads them: none is
//! read as one of git's options (`--output=…` writes a file); a branch, tag or start point is a plain name, never a
//! refspec (`+a:b` would force-overwrite a ref, `*` match every branch) or a revision expression; a remote is one the
//! repository lists, never a URL or a folder; and a path stays inside the repository.

use std::path::{Component, Path};

use super::command::Git;
use crate::app_mcp::ToolError;

/// `value`, a revision or range the chat named for `argument`, which git reads only as a revision (after
/// `--end-of-options`, or as a ref it compares): not empty, not an option, one line.
pub(super) fn revision<'a>(argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let value = value.trim();
  if value.is_empty() || value.starts_with('-') || value.contains(['\n', '\r', '\0']) {
    return Err(ToolError::InvalidArguments(format!(
      "{argument}: {value:?} is not a revision git reads"
    )));
  }
  Ok(value)
}

/// `value`, a plain name the chat named for `argument`: no refspec, glob or revision expression (`+ : * ? [ ^ ~ \`,
/// `..`, `@{`, `@` alone), no space or control character, no leading `-` or `/`, no trailing `/`, `.` or `.lock`.
pub(super) fn plain<'a>(argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let refused = value.is_empty()
    || value == "@"
    || value.starts_with(['-', '/'])
    || value.ends_with(['/', '.'])
    || value.ends_with(".lock")
    || value.contains("..")
    || value.contains("@{")
    || value.contains("//")
    || value
      .chars()
      .any(|character| character.is_whitespace() || character.is_control() || "+:*?[^~\\".contains(character));
  if refused {
    return Err(ToolError::InvalidArguments(format!(
      "{argument}: {value:?} is not a plain name"
    )));
  }
  Ok(value)
}

/// `value`, a branch's name the chat named for `argument`: a plain name git takes as a branch
/// (`git check-ref-format --branch`).
pub(super) fn branch<'a>(git: &Git, argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let name = plain(argument, value)?;
  if git.succeeds(&["check-ref-format", "--branch", name])? {
    return Ok(name);
  }
  Err(ToolError::InvalidArguments(format!(
    "{argument}: {name:?} is not a branch name git takes"
  )))
}

/// `value`, a tag's name the chat named for `argument`: a plain name git takes as a tag
/// (`git check-ref-format refs/tags/<name>`).
pub(super) fn tag<'a>(git: &Git, argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let name = plain(argument, value)?;
  if git.succeeds(&["check-ref-format", &format!("refs/tags/{name}")])? {
    return Ok(name);
  }
  Err(ToolError::InvalidArguments(format!(
    "{argument}: {name:?} is not a tag name git takes"
  )))
}

/// `value`, a start point the chat named for `argument`: a plain name or a commit's id that names a commit.
pub(super) fn start<'a>(git: &Git, argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let name = plain(argument, value)?;
  if commit_of(git, name)?.is_some() {
    return Ok(name);
  }
  Err(ToolError::InvalidArguments(format!(
    "{argument}: {name:?} names no commit"
  )))
}

/// `value`, a remote the chat named for `argument`: only one `git remote` lists, never a URL or a folder.
pub(super) fn remote<'a>(git: &Git, argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  // A plain name holds no `:`, so it is never a URL with credentials, nor a scp-like address.
  let value = plain(argument, value)?;
  let listed = git.ok(&["remote"])?;
  if listed.lines().any(|remote| remote.trim() == value) {
    return Ok(value);
  }
  Err(ToolError::InvalidArguments(format!(
    "{argument}: {value:?} is not one of the repository's remotes (git_remotes lists them)"
  )))
}

/// `value`, a path the chat named for `argument`: relative to the repository's top, inside it (no root, drive or
/// `..`), never read as an option; it always follows `--` or a revision's `:`.
pub(super) fn path<'a>(argument: &str, value: &'a str) -> Result<&'a str, ToolError> {
  let inside = Path::new(value)
    .components()
    .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
  if value.trim().is_empty() || value.contains(['\0', ':']) || value.starts_with(['-', '/', '\\']) || !inside {
    return Err(ToolError::InvalidArguments(format!(
      "{argument}: {value:?} is not a path inside the repository"
    )));
  }
  Ok(value)
}

/// The paths the chat named for `argument`, each [`path`].
pub(super) fn paths<'a>(argument: &str, values: &'a [String]) -> Result<Vec<&'a str>, ToolError> {
  values.iter().map(|value| path(argument, value)).collect()
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
