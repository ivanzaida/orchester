//! Checkpoint commits and merges: everything a step changed in its worktree, committed on the
//! worktree's branch (or the commits its agent made there itself), a step branch merged back
//! into the run's branch, and the moved base merged into it (ORC-151) or into a writing agent's worktree, its conflicts
//! left there for the agent — with the repository's git identity.

use std::{collections::BTreeSet, path::Path};

use orchester_core::{revision::Revision, run::BaseMerge};

use crate::{git::Git, worktree::WorktreeError};

/// Commits every change in `worktree` with `message`, and the merge in progress there, whose conflicts its agent
/// resolved, even when it changed nothing; `None` when there was nothing to commit.
pub(super) fn commit(worktree: &Path, message: &str) -> Result<Option<Revision>, WorktreeError> {
  let git = Git::new(worktree);
  if !merging(&git)?
    && git
      .stdout(&["status", "--porcelain", "--untracked-files=all"])?
      .is_empty()
  {
    return Ok(None);
  }
  git.stdout(&["add", "--all"])?;
  git.stdout(&["commit", "--quiet", "-m", message])?;
  head(worktree).map(Some)
}

/// What a step left in its worktree when it ended.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Saved {
  /// Its work is at this commit: what it left uncommitted, committed now, or else the commits its agent made itself.
  At(Revision),
  /// Nothing new.
  Nothing,
  /// Its agent's commits are on `head` (another branch, or a detached HEAD), not on `branch`, the worktree's: they are
  /// not taken as its work.
  Elsewhere { head: String, branch: String },
  /// A merge of the base branch the runner began for its agent is in progress there, and these files it conflicted in
  /// still hold conflict markers: nothing is committed, and the merge stays in progress for its agent to resolve.
  Conflicted(Vec<String>),
}

/// Commits every change in `worktree` with `message`; with nothing to commit, takes the commits the step's agent made
/// itself on `branch`, the worktree's, when its HEAD is none of the `known` revisions (where the step started, the
/// run's last checkpoint, the run's start). A merge in progress is committed only once no conflict marker is left.
pub(super) fn save(
  worktree: &Path,
  message: &str,
  (branch, known): (&str, &[&Revision]),
) -> Result<Saved, WorktreeError> {
  let left = conflicts_left(worktree)?;
  if !left.is_empty() {
    return Ok(Saved::Conflicted(left));
  }
  if let Some(revision) = commit(worktree, message)? {
    return Ok(Saved::At(revision));
  }
  let head = head(worktree)?;
  if known.contains(&&head) {
    return Ok(Saved::Nothing);
  }
  let on = Git::new(worktree).attempt(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
  if on.as_deref() == Ok(branch) {
    return Ok(Saved::At(head));
  }
  Ok(Saved::Elsewhere {
    head: on.unwrap_or_else(|_| "a detached HEAD".to_owned()),
    branch: branch.to_owned(),
  })
}

/// The commit checked out in `worktree`.
pub(super) fn head(worktree: &Path) -> Result<Revision, WorktreeError> {
  let head = Git::new(worktree).stdout(&["rev-parse", "HEAD"])?;
  Revision::new(&head).map_err(|_| WorktreeError::UnexpectedRevision(head))
}

/// How merging a step's branch back, or the moved base, went.
pub(super) enum Merge {
  /// The run's branch is at this commit now: the step branch fast-forwarded it, a merge commit
  /// joined them, or it had nothing new.
  Merged(Revision),
  /// Both changed these files: the merge stopped at their conflicts. [`merge`] and [`merge_moved_base`] undo it, so
  /// the branch is as it was; [`begin_base_merge`] leaves it in progress.
  Conflict(Vec<String>),
}

impl Merge {
  /// The merge as an agent's prompt tells it, of the base merged into its worktree.
  pub(super) fn as_prompt(&self) -> BaseMerge<'_> {
    match self {
      Self::Merged(revision) => BaseMerge::Merged(revision),
      Self::Conflict(files) => BaseMerge::Conflicted(files),
    }
  }
}

/// Merges `branch` into the branch checked out in `worktree` (the run's), with `message` when it
/// takes a merge commit.
pub(super) fn merge(worktree: &Path, branch: &str, message: &str) -> Result<Merge, WorktreeError> {
  merge_with(worktree, &["merge", "--no-edit", "-m", message, branch], branch)
}

/// Merges `revision`, where the run's base branch moved, into the branch checked out in `worktree` (the run's) with a
/// merge commit of `message`, never a fast-forward, as a writing agent is told to merge it (ORC-151).
pub(super) fn merge_moved_base(worktree: &Path, revision: &Revision, message: &str) -> Result<Merge, WorktreeError> {
  let revision = revision.as_str();
  merge_with(
    worktree,
    &["merge", "--no-ff", "--no-edit", "-m", message, revision],
    revision,
  )
}

/// Merges `revision`, the tip of the run's base branch, into the branch checked out in `worktree`, a writing step's,
/// before its agent starts on it — one that reads that the base moved, or one a merge's conflicts were returned to —
/// with a merge commit of `message`. A sandboxed agent cannot write the repository's git data, so the runner merges
/// for it: a merge that conflicts stays in progress, with git's conflict markers in the files, for the agent to
/// resolve and [`save`] to commit with its work.
pub(super) fn begin_base_merge(worktree: &Path, revision: &Revision, message: &str) -> Result<Merge, WorktreeError> {
  let revision = revision.as_str();
  start_merge(
    worktree,
    &["merge", "--no-ff", "--no-edit", "-m", message, revision],
    revision,
  )
}

/// The commit `branch` (a local branch, as a run's base branch is) points at, seen from `worktree`.
pub(super) fn branch_tip(worktree: &Path, branch: &str) -> Result<Revision, WorktreeError> {
  let tip = Git::new(worktree).stdout(&[
    "rev-parse",
    "--verify",
    "--quiet",
    &format!("refs/heads/{branch}^{{commit}}"),
  ])?;
  Revision::new(&tip).map_err(|_| WorktreeError::UnexpectedRevision(tip))
}

/// `git merge` with `args` of `what` in `worktree`; a merge that conflicts is undone.
fn merge_with(worktree: &Path, args: &[&str], what: &str) -> Result<Merge, WorktreeError> {
  let merged = start_merge(worktree, args, what)?;
  if let Merge::Conflict(_) = merged {
    Git::new(worktree).stdout(&["merge", "--abort"])?;
  }
  Ok(merged)
}

/// `git merge` with `args` of `what` in `worktree`; a merge that conflicts is left in progress.
fn start_merge(worktree: &Path, args: &[&str], what: &str) -> Result<Merge, WorktreeError> {
  let git = Git::new(worktree);
  if let Err(refusal) = git.attempt(args)? {
    // A conflict leaves a merge in progress; a refusal before merging (such as local changes in
    // the way) does not, and is a failure of git rather than of the step.
    if !merging(&git)? {
      return Err(WorktreeError::Git {
        command: format!("merge {what}"),
        code: None,
        stderr: refusal,
      });
    }
    return Ok(Merge::Conflict(unmerged(&git)?));
  }
  head(worktree).map(Merge::Merged)
}

/// Whether a merge is in progress in `worktree`.
pub(super) fn in_merge(worktree: &Path) -> Result<bool, WorktreeError> {
  merging(&Git::new(worktree))
}

/// The files git has not merged yet of the merge in progress in `worktree`, in path order, conflict markers left in
/// them or not: its agent edits them but cannot mark them merged, so they stay listed until the merge is committed.
/// `None` when no merge is in progress.
pub(super) fn merge_in_progress(worktree: &Path) -> Result<Option<Vec<String>>, WorktreeError> {
  let git = Git::new(worktree);
  if !merging(&git)? {
    return Ok(None);
  }
  unmerged(&git).map(Some)
}

fn merging(git: &Git<'_>) -> Result<bool, WorktreeError> {
  git.answer(&["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
}

/// The files git has not merged yet, in path order.
fn unmerged(git: &Git<'_>) -> Result<Vec<String>, WorktreeError> {
  Ok(
    paths(git, &["diff", "--name-only", "--diff-filter=U"])?
      .into_iter()
      .collect(),
  )
}

/// The files a merge in progress in `worktree` conflicted in that still hold a conflict marker, in path order; none
/// when no merge is in progress.
pub(super) fn conflicts_left(worktree: &Path) -> Result<Vec<String>, WorktreeError> {
  let git = Git::new(worktree);
  if !merging(&git)? {
    return Ok(Vec::new());
  }
  Ok(
    unmerged(&git)?
      .into_iter()
      .filter(|path| holds_marker(&worktree.join(path)))
      .collect(),
  )
}

/// Whether the file at `path` has a line git starts or ends a conflict with. One that cannot be read (a file the
/// agent deleted to resolve its conflict) has none.
fn holds_marker(path: &Path) -> bool {
  std::fs::read(path).is_ok_and(|content| {
    content
      .split(|byte| *byte == b'\n')
      .any(|line| line.starts_with(b"<<<<<<<") || line.starts_with(b">>>>>>>"))
  })
}

/// The files that both the work in `worktree` — committed, uncommitted and untracked — and `revision`, where the run's
/// base branch moved, changed since they forked, in path order (ORC-151).
pub(super) fn overlap(worktree: &Path, revision: &Revision) -> Result<Vec<String>, WorktreeError> {
  let git = Git::new(worktree);
  let fork = git.stdout(&["merge-base", "HEAD", revision.as_str()])?;
  let theirs = paths(&git, &["diff", "--name-only", "--no-renames", &fork, revision.as_str()])?;
  let mut ours = paths(&git, &["diff", "--name-only", "--no-renames", &fork])?;
  ours.extend(paths(&git, &["ls-files", "--others", "--exclude-standard"])?);
  Ok(ours.intersection(&theirs).cloned().collect())
}

/// The paths `git` lists with `args`, one per line, as written.
fn paths(git: &Git<'_>, args: &[&str]) -> Result<BTreeSet<String>, WorktreeError> {
  let args: Vec<&str> = ["-c", "core.quotepath=false"]
    .into_iter()
    .chain(args.iter().copied())
    .collect();
  Ok(
    git
      .stdout(&args)?
      .lines()
      .map(str::trim)
      .filter(|path| !path.is_empty())
      .map(str::to_owned)
      .collect(),
  )
}
