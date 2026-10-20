//! Where a run's own work starts (ORC-60). A run records the commit it started from, but its branch may take newer
//! commits of the branch it merges into since: a fast-forward onto it (an agent told to merge the base before it had
//! committed anything), a merge of it, a rebase onto it. Those commits are the target's and never part of the run's
//! result, so its work starts at the newest commit git proves the branch took in from the target ([`fork_point`]):
//! one a fast-forward of the branch moved it to, as its reflog names it, or the newest commit the branch and the
//! target share. A commit of the run's own at or below it makes it no base: then the target holds work of the run's,
//! which stays part of its result. The run's own commits are its checkpoints and those its branch's reflog shows were
//! made on it ([`made_on_branch`]); a commit the target has as a cherry-pick is another commit, and is no reason.
//! The recorded start stays the run's for its history and display.

use std::{collections::HashSet, path::Path};

use super::{GitError, RunWork, git::Git};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// The git queries [`fork_point`] asks, of the repository the run's branch and its target are in.
pub trait Ancestry {
  type Error;

  /// Whether `ancestor` is reachable from `descendant`.
  fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, Self::Error>;

  /// The best common ancestor of `one` and `other`; `None` when they share none.
  fn merge_base(&self, one: &str, other: &str) -> Result<Option<String>, Self::Error>;

  /// The commits reachable from `to` and not from `from`.
  fn commits_between(&self, from: &str, to: &str) -> Result<Vec<String>, Self::Error>;

  /// The entries of `branch`'s reflog, newest first: the commit each moved it to, and its subject.
  fn branch_log(&self, branch: &BranchName) -> Result<Vec<(String, String)>, Self::Error>;
}

/// A run's branch as [`fork_point`] reads it.
#[derive(Clone, Copy, Debug)]
pub struct RunFork<'a> {
  pub branch: &'a BranchName,
  /// The commit the run recorded it started from.
  pub base: &'a str,
  /// The commit its work ends at: its branch tip or its last checkpoint.
  pub head: &'a str,
  /// The commit of the branch it merges into.
  pub target: &'a str,
  pub checkpoints: &'a [Revision],
  /// A base proven before (`local::RunRebase`), which still counts once the branch and its reflog are gone, as after
  /// the run was merged.
  pub proven: Option<&'a str>,
}

/// Whether a reflog entry with `subject` made the commit it moved its branch to on that branch: a commit, an amend,
/// a merge commit, a cherry-pick, a revert, a rebase, `git am`; not a branch created, a fast-forward or a reset.
pub fn made_on_branch(subject: &str) -> bool {
  const MADE: [&str; 7] = ["commit", "cherry-pick", "revert", "merge ", "pull", "rebase", "am:"];
  !fast_forward(subject) && MADE.iter().any(|made| subject.starts_with(made))
}

/// Whether a reflog entry with `subject` fast-forwarded its branch: `merge <base>: Fast-forward`, `pull: Fast-forward`.
fn fast_forward(subject: &str) -> bool {
  subject.ends_with(": Fast-forward")
}

/// The newest commit of the target `fork`'s branch took in, newer than its recorded base, with no commit of the
/// run's own at or below it; `None` when there is none and the recorded base stands.
pub fn fork_point<A: Ancestry>(git: &A, fork: &RunFork<'_>) -> Result<Option<String>, A::Error> {
  let log = git.branch_log(fork.branch)?;
  let own: HashSet<&str> = fork
    .checkpoints
    .iter()
    .map(Revision::as_str)
    .chain(
      log
        .iter()
        .filter(|(_, subject)| made_on_branch(subject))
        .map(|(commit, _)| commit.as_str()),
    )
    .collect();
  let mut candidates: Vec<String> = log
    .iter()
    .filter(|(_, subject)| fast_forward(subject))
    .map(|(commit, _)| commit.clone())
    .chain(fork.proven.map(str::to_owned))
    .collect();
  candidates.extend(git.merge_base(fork.head, fork.target)?);
  let mut best: Option<String> = None;
  for candidate in candidates {
    if candidate == fork.base || best.as_deref() == Some(candidate.as_str()) {
      continue;
    }
    if let Some(best) = &best
      && !git.is_ancestor(best, &candidate)?
    {
      continue;
    }
    let taken_in = git.is_ancestor(fork.base, &candidate)?
      && git.is_ancestor(&candidate, fork.head)?
      && git.is_ancestor(&candidate, fork.target)?;
    if !taken_in {
      continue;
    }
    let below = git.commits_between(fork.base, &candidate)?;
    if below.iter().any(|commit| own.contains(commit.as_str())) {
      continue;
    }
    best = Some(candidate);
  }
  Ok(best)
}

/// The commit `work`'s changes up to `head` start from in the repository at `repo`, read with the `git` program:
/// [`fork_point`] against its base branch, else the revision it started from. `checkpoints` are the run's, and
/// `proven` the base a proof recorded for it before.
pub fn own_base(
  runner: &impl CommandRunner,
  (program, repo): (&Path, &Path),
  (work, head): (&RunWork, &Revision),
  (checkpoints, proven): (&[Revision], Option<&Revision>),
) -> Result<Revision, GitError> {
  let git = Git::new(runner, program, repo);
  let Some(into) = &work.into else {
    return Ok(work.base.clone());
  };
  let Some(target) = git.commit(&format!("refs/heads/{into}"))? else {
    return Ok(work.base.clone());
  };
  let fork = RunFork {
    branch: &work.branch,
    base: work.base.as_str(),
    head: head.as_str(),
    target: &target,
    checkpoints,
    proven: proven.map(Revision::as_str),
  };
  match fork_point(&git, &fork)? {
    Some(commit) => Revision::new(&commit).map_err(|_| GitError::Unrecognized {
      expected: "a commit",
      detail: commit,
    }),
    None => Ok(work.base.clone()),
  }
}

impl<R: CommandRunner> Ancestry for Git<'_, R> {
  type Error = GitError;

  fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, GitError> {
    Self::is_ancestor(self, ancestor, descendant)
  }

  fn merge_base(&self, one: &str, other: &str) -> Result<Option<String>, GitError> {
    let output = self.output(&["merge-base", one, other])?;
    // Git exits with 1 for commits that share no ancestor.
    Ok(
      output
        .success()
        .then(|| output.stdout.trim().to_owned())
        .filter(|commit| !commit.is_empty()),
    )
  }

  fn commits_between(&self, from: &str, to: &str) -> Result<Vec<String>, GitError> {
    let excluded = format!("^{from}");
    let listed = self.stdout(&["rev-list", to, &excluded])?;
    Ok(listed.lines().map(str::to_owned).collect())
  }

  fn branch_log(&self, branch: &BranchName) -> Result<Vec<(String, String)>, GitError> {
    let reference = format!("refs/heads/{branch}");
    let output = self.output(&["reflog", "show", "--format=%H%x09%gs", &reference, "--"])?;
    // A branch without a reflog proves nothing either way.
    if !output.success() {
      return Ok(Vec::new());
    }
    Ok(parse_log(&output.stdout))
  }
}

/// The entries `git reflog show --format=%H%x09%gs` printed.
pub fn parse_log(text: &str) -> Vec<(String, String)> {
  text
    .lines()
    .filter_map(|line| {
      let (commit, subject) = line.split_once('\t')?;
      Some((commit.trim().to_owned(), subject.to_owned()))
    })
    .collect()
}
