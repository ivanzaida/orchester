//! What the merge queue asks git (`docs/design/orchester-workspace.md` › *Delivery*): how the person's checkout
//! stands on the base branch ([`checkout`]), which delivery never merges into while it holds uncommitted changes; and
//! syncing a run with its base branch in the run's own worktree ([`sync`]): the current base is merged into the run's
//! branch there, and a merge that conflicts is put back, so that the worktree is as the run left it for the step that
//! resolves the conflicts.

use std::path::Path;

use super::{
  GitError,
  git::Git,
  merge::in_progress,
  resolve::{self, ResolveError, Started},
};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// How the person's checkout stands for delivering into `base`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checkout {
  /// It has the base branch checked out, with these uncommitted files (`git status` lines: the status, then the
  /// path); none when it is clean.
  OnBase { uncommitted: Vec<String> },
  /// It has another branch checked out; `None` on a detached HEAD.
  OtherBranch(Option<BranchName>),
  /// A merge, cherry-pick or revert is in progress in it.
  InProgress,
}

/// How the person's checkout at `repo` stands for delivering into `base`. Ignored files are not uncommitted.
pub fn checkout(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, base): (&Path, &BranchName),
) -> Result<Checkout, GitError> {
  let git = Git::new(runner, git, repo);
  let current = git.checked_out_branch()?;
  if current.as_ref() != Some(base) {
    return Ok(Checkout::OtherBranch(current));
  }
  if in_progress(&git)? {
    return Ok(Checkout::InProgress);
  }
  // Not trimmed: the first entry's status can start with a space.
  let output = git.output(&["status", "--porcelain=v1", "-z", "--untracked-files=normal"])?;
  if !output.success() {
    return Err(GitError::Failed(super::git::message(&output)));
  }
  Ok(Checkout::OnBase {
    uncommitted: status_lines(&output.stdout),
  })
}

/// `git status --porcelain=v1 -z` as lines of a status and a path (`M src/lib.rs`, `?? notes.txt`); a rename or copy
/// names its new path.
fn status_lines(status: &str) -> Vec<String> {
  let mut lines = Vec::new();
  let mut entries = status.split('\0').filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let (Some(code), Some(path)) = (entry.get(..2), entry.get(3..)) else {
      continue;
    };
    if code.contains(['R', 'C']) {
      entries.next();
    }
    lines.push(format!("{} {path}", code.trim()));
  }
  lines
}

/// How syncing a run's branch with its base branch went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Synced {
  /// The branch holds the base branch at `base` now and its tip is `tip`: it already held it, or git merged it
  /// without a conflict and committed the merge.
  Clean { base: Revision, tip: Revision },
  /// Merging the base branch at `base` conflicts in `files`; the merge was put back.
  Conflicts { base: Revision, files: Vec<String> },
}

/// Merges `base`, as the repository has it now, into `branch`, the run's branch checked out in its `worktree`, which
/// must have no uncommitted changes. A merge that conflicts is aborted, leaving the worktree as it was.
pub fn sync(
  runner: &impl CommandRunner,
  git: &Path,
  (worktree, base, branch): (&Path, &BranchName, &BranchName),
) -> Result<Synced, ResolveError> {
  let in_worktree = Git::new(runner, git, worktree);
  let base_head = in_worktree
    .commit(&format!("refs/heads/{base}"))?
    .ok_or_else(|| GitError::Failed(format!("the base branch {base} does not exist")))?;
  let tip = in_worktree
    .commit("HEAD")?
    .ok_or_else(|| GitError::Failed("HEAD names no commit".to_owned()))?;
  if in_worktree.is_ancestor(&base_head, &tip)? {
    return Ok(Synced::Clean {
      base: revision(&base_head)?,
      tip: revision(&tip)?,
    });
  }
  match resolve::start(runner, git, worktree, (base, branch, &revision(&tip)?))? {
    Started::Merged(merged) => Ok(Synced::Clean {
      base: merged.base,
      tip: merged.tip,
    }),
    Started::Conflicts(files) => {
      resolve::abort(runner, git, worktree)?;
      Ok(Synced::Conflicts {
        base: revision(&base_head)?,
        files,
      })
    }
  }
}

/// The commit `HEAD` is at in `worktree`.
pub fn head(runner: &impl CommandRunner, git: &Path, worktree: &Path) -> Result<Revision, GitError> {
  let head = Git::new(runner, git, worktree)
    .commit("HEAD")?
    .ok_or_else(|| GitError::Failed("HEAD names no commit".to_owned()))?;
  revision(&head)
}

/// Deletes `branch` in the repository at `repo`, whose work landed squashed: git does not see it merged, so
/// clearing up after the merge keeps it.
pub fn delete_branch(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, branch): (&Path, &BranchName),
) -> Result<(), GitError> {
  Git::new(runner, git, repo)
    .stdout(&["branch", "--quiet", "-D", "--end-of-options", branch.as_str()])
    .map(drop)
}

fn revision(sha: &str) -> Result<Revision, GitError> {
  Revision::new(sha).map_err(|_| GitError::Unrecognized {
    expected: "a commit",
    detail: sha.to_owned(),
  })
}
