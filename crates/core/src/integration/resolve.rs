//! Resolving by hand what conflicted when a run was merged into its base branch: the base branch is merged into
//! the run's branch in the run's own worktree — never in the person's checkout — and git leaves its conflict
//! markers there for the person to resolve in their editor ([`start`]). While that merge is in progress the
//! worktree tells which files still conflict ([`resolution`]); [`finish`] commits the merge once no conflicted file
//! holds a marker, after which the run's branch holds the base branch and merges into it without conflicts, and
//! [`abort`] puts the worktree back as it was. A worktree with uncommitted work is not merged into: it is the
//! agent's, and a merge would mix the two.

use std::{fs, path::Path};

use thiserror::Error;

use super::{
  GitError,
  git::{Git, message},
};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// What a line of a file that still holds a conflict starts with: git's markers around and inside a conflict
/// (`diff3` style included). A line of `=======` alone is not looked for: Markdown underlines a heading with it.
const MARKERS: [&str; 3] = ["<<<<<<<", ">>>>>>>", "|||||||"];

/// A merge in progress in a run's worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
  /// Every file that conflicted in this merge, as `git` names them from the worktree's root: those git still has
  /// as unmerged in the index first, then those marked resolved since (`git add`).
  pub conflicted: Vec<String>,
  /// The files of [`Self::conflicted`] that still hold a conflict marker.
  pub markers: Vec<String>,
}

/// The merge of the base branch committed in the run's worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedIn {
  /// The commit of the base branch that was merged in: the run's own commits sit on it now.
  pub base: Revision,
  /// The merge commit, the run's branch's tip.
  pub tip: Revision,
}

/// How merging the base branch into the run's branch went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Started {
  /// Git merged without a conflict and committed the merge.
  Merged(MergedIn),
  /// These files conflict; the merge waits in the worktree with git's markers in them.
  Conflicts(Vec<String>),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ResolveError {
  #[error(transparent)]
  Git(#[from] GitError),
  /// The worktree has changes nobody committed: they are the agent's, and merging would mix them in.
  #[error("the worktree has uncommitted changes: {}", .0.join(", "))]
  Uncommitted(Vec<String>),
  /// The worktree is not on the run's branch, so merging there would not change the run's work.
  #[error("the worktree is not on the run's branch")]
  NotOnBranch,
  /// A merge is in progress in the worktree already.
  #[error("a merge is in progress in the worktree already")]
  InMerge,
  /// No merge is in progress in the worktree.
  #[error("no merge is in progress in the worktree")]
  NotInMerge,
  /// These conflicted files still hold conflict markers.
  #[error("conflict markers are left in {}", .0.join(", "))]
  MarkersLeft(Vec<String>),
  /// A conflicted file could not be read to look for markers.
  #[error("{path} cannot be read: {detail}")]
  Unreadable { path: String, detail: String },
}

/// The merge in progress in `worktree`, if any, with the files that still hold a marker.
pub fn resolution(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
) -> Result<Option<Resolution>, ResolveError> {
  let in_worktree = Git::new(runner, git, worktree);
  if !in_merge(&in_worktree)? {
    return Ok(None);
  }
  let mut conflicted = unmerged(&in_worktree)?;
  for resolved in listed(&in_worktree, &["ls-files", "--resolve-undo", "-z"])? {
    if !conflicted.contains(&resolved) {
      conflicted.push(resolved);
    }
  }
  let markers = with_markers(worktree, conflicted.iter())?;
  Ok(Some(Resolution { conflicted, markers }))
}

/// Merges `base` into `branch`, the run's branch checked out in its `worktree`, leaving the conflicts there for a
/// person to resolve.
pub fn start(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
  (base, branch): (&BranchName, &BranchName),
) -> Result<Started, ResolveError> {
  let git = Git::new(runner, git, worktree);
  if in_merge(&git)? {
    return Err(ResolveError::InMerge);
  }
  if git.checked_out_branch()?.as_ref() != Some(branch) {
    return Err(ResolveError::NotOnBranch);
  }
  let uncommitted = git.uncommitted()?;
  if !uncommitted.is_empty() {
    return Err(ResolveError::Uncommitted(uncommitted));
  }
  let title = format!("Merge {} into {}", base.as_str(), branch.as_str());
  let base = format!("refs/heads/{}", base.as_str());
  let output = git.output(&["merge", "--no-ff", "--no-edit", "-m", &title, "--end-of-options", &base])?;
  if output.success() {
    return Ok(Started::Merged(merged_in(&git)?));
  }
  let conflicts = unmerged(&git)?;
  if conflicts.is_empty() || !in_merge(&git)? {
    return Err(GitError::Failed(message(&output)).into());
  }
  Ok(Started::Conflicts(conflicts))
}

/// Commits the merge in progress in `worktree` once no file that conflicted holds a marker: the conflicted files
/// and the tracked files changed beside them are committed as the merge.
pub fn finish(runner: &impl CommandRunner, git: &Path, worktree: &Path) -> Result<MergedIn, ResolveError> {
  let resolution = resolution(runner, git, worktree)?.ok_or(ResolveError::NotInMerge)?;
  if !resolution.markers.is_empty() {
    return Err(ResolveError::MarkersLeft(resolution.markers));
  }
  let git = Git::new(runner, git, worktree);
  let mut add = vec!["add", "-A", "--"];
  add.extend(resolution.conflicted.iter().map(String::as_str));
  git.stdout(&add)?;
  git.stdout(&["add", "-u"])?;
  git.stdout(&["commit", "--no-edit"])?;
  Ok(merged_in(&git)?)
}

/// Ends the merge in progress in `worktree` and puts the worktree back as it was before it.
pub fn abort(runner: &impl CommandRunner, git: &Path, worktree: &Path) -> Result<(), ResolveError> {
  let git = Git::new(runner, git, worktree);
  if !in_merge(&git)? {
    return Err(ResolveError::NotInMerge);
  }
  git.stdout(&["merge", "--abort"])?;
  Ok(())
}

/// Whether `text` still holds a conflict marker at the start of a line.
pub fn holds_markers(text: &str) -> bool {
  text
    .lines()
    .any(|line| MARKERS.iter().any(|marker| line.starts_with(marker)))
}

/// The files of `paths` in `worktree` that still hold a conflict marker; a file that is gone holds none.
fn with_markers<'a>(worktree: &Path, paths: impl Iterator<Item = &'a String>) -> Result<Vec<String>, ResolveError> {
  let mut left = Vec::new();
  for path in paths {
    let bytes = match fs::read(worktree.join(path)) {
      Ok(bytes) => bytes,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => {
        return Err(ResolveError::Unreadable {
          path: path.clone(),
          detail: error.to_string(),
        });
      }
    };
    if holds_markers(&String::from_utf8_lossy(&bytes)) {
      left.push(path.clone());
    }
  }
  Ok(left)
}

/// The merge commit at the worktree's `HEAD` and the base commit it merged in, its second parent.
fn merged_in<R: CommandRunner>(git: &Git<'_, R>) -> Result<MergedIn, GitError> {
  let revision = |name: &str| {
    let commit = git.stdout(&["rev-parse", "--verify", "--end-of-options", name])?;
    Revision::new(&commit).map_err(|_| GitError::Unrecognized {
      expected: "a commit",
      detail: commit,
    })
  };
  Ok(MergedIn {
    base: revision("HEAD^2")?,
    tip: revision("HEAD")?,
  })
}

fn in_merge<R: CommandRunner>(git: &Git<'_, R>) -> Result<bool, GitError> {
  git.succeeds(&["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
}

/// The files with unmerged entries in the index.
fn unmerged<R: CommandRunner>(git: &Git<'_, R>) -> Result<Vec<String>, GitError> {
  listed(git, &["ls-files", "--unmerged", "-z"])
}

/// The paths of an `ls-files -z` listing with stage information (`<mode> <object> <stage>\t<path>`), each once.
fn listed<R: CommandRunner>(git: &Git<'_, R>, args: &[&str]) -> Result<Vec<String>, GitError> {
  let output = git.output(args)?;
  if !output.success() {
    return Err(GitError::Failed(message(&output)));
  }
  let mut paths: Vec<String> = Vec::new();
  for entry in output.stdout.split('\0').filter(|entry| !entry.is_empty()) {
    let path = entry.split_once('\t').map_or(entry, |(_, path)| path);
    if !paths.iter().any(|known| known == path) {
      paths.push(path.to_owned());
    }
  }
  Ok(paths)
}
