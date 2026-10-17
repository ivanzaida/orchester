//! The merge commit, made in a temporary worktree so that neither the merge nor what stops it
//! reaches the user's checkout, whose uncommitted work stays out of it. The worktree is
//! `<git common dir>/orchester-merge-<run id>`: inside the repository's own git folder, so on the
//! same disk as the checkout, out of every working tree and file watcher, and named by the run, so
//! one a killed merge left is the next merge of that run's to remove. It is detached at the
//! checkout's commit, filled with `git reset --hard` (which, unlike a checkout, runs no
//! `post-checkout` hook), and removed again on every path, so no ref and no worktree stays.
//!
//! The merge there runs the repository's hooks as a merge in the checkout would (`pre-merge-commit`,
//! `prepare-commit-msg`, `commit-msg`, `post-merge`) with the same identity: worktrees share the
//! configuration and the hooks. Their working directory is the temporary worktree, and
//! `post-merge` runs again in the checkout when the merge is brought in.

use std::path::Path;

use super::MergeError;
use crate::{
  command::CommandRunner,
  id::RunId,
  integration::git::{Git, message},
};

/// Merges `head` into `base` with a merge commit titled `title`, the merge of `run`; the commit.
pub(super) fn merge_commit<R: CommandRunner>(
  git: &Git<'_, R>,
  run: RunId,
  (base, head): (&str, &str),
  title: &str,
) -> Result<String, MergeError> {
  let common = git.stdout(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
  let path = format!("{common}/orchester-merge-{run}");
  remove(git, &path)?;
  let made = merge_in(git, &path, (base, head), title);
  let removed = remove(git, &path);
  let commit = made?;
  removed?;
  Ok(commit)
}

fn merge_in<R: CommandRunner>(
  git: &Git<'_, R>,
  path: &str,
  (base, head): (&str, &str),
  title: &str,
) -> Result<String, MergeError> {
  // `--force` takes over the place of one a killed merge left registered with its folder gone.
  git.stdout(&[
    "worktree",
    "add",
    "--quiet",
    "--force",
    "--detach",
    "--no-checkout",
    path,
    base,
  ])?;
  let temporary = git.at(Path::new(path));
  temporary.stdout(&["reset", "--hard", "--quiet"])?;
  let output = temporary.output(&[
    "merge",
    "--no-ff",
    "--no-edit",
    "--no-overwrite-ignore",
    "-m",
    title,
    head,
  ])?;
  if !output.success() {
    return Err(stopped(&temporary, message(&output)));
  }
  Ok(temporary.stdout(&["rev-parse", "--verify", "HEAD"])?)
}

/// Why the merge in `temporary` stopped: the files that conflict, else git's message `said`.
fn stopped<R: CommandRunner>(temporary: &Git<'_, R>, said: String) -> MergeError {
  match temporary.stdout(&["diff", "--name-only", "--diff-filter=U", "-z"]) {
    Ok(files) if !files.is_empty() => MergeError::Conflict {
      files: files
        .split('\0')
        .filter(|file| !file.is_empty())
        .map(str::to_owned)
        .collect(),
    },
    Ok(_) => MergeError::Refused(said),
    Err(error) => error.into(),
  }
}

/// Removes the temporary worktree at `path` with what git keeps of it, when its folder is there.
/// A folder git does not know as a worktree is not removed: git's refusal says so.
fn remove<R: CommandRunner>(git: &Git<'_, R>, path: &str) -> Result<(), MergeError> {
  if Path::new(path).exists() {
    git.stdout(&["worktree", "remove", "--force", path])?;
  }
  Ok(())
}
