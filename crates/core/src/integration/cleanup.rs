//! What a run left in the repository, and clearing it (`docs/architecture/storage.md`, "Agent
//! worktrees"): the run's worktree and the worktrees of steps that ran beside others and were kept
//! (a step that failed or was skipped keeps its own), under `.orchester/worktrees/`, and the run's
//! branch and step branches. A worktree with uncommitted changes is removed only when a person saw
//! those very changes and confirmed discarding them; a branch is deleted only when its commits are
//! on the run's base branch or the branch the checkout has now, so nothing committed is lost.
//! Every other branch is kept.

use std::path::Path;

use thiserror::Error;

use super::{GitError, git::Git};
use crate::{
  command::CommandRunner,
  id::StepExecutionId,
  run::{BranchName, Run},
};

/// Where runs' worktrees are, relative to the repository.
pub const WORKTREES_DIR: &str = ".orchester/worktrees";

/// Whose worktree it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorktreeOwner {
  Run,
  /// A step execution that ran beside other steps of the run.
  Step(StepExecutionId),
}

/// A worktree of the run, as git lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftWorktree {
  /// As git prints it: absolute, with `/` separators on every platform.
  pub path: String,
  pub owner: WorktreeOwner,
  /// The branch checked out there; `None` on a detached HEAD.
  pub branch: Option<BranchName>,
  /// Files with changes not committed there, untracked ones included.
  pub uncommitted: Vec<String>,
  /// Its folder is gone; git still lists it until it is pruned.
  pub missing: bool,
}

/// A branch of the run: its own or a step's (`<run-branch>--<step>-<attempt>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftBranch {
  pub name: BranchName,
  /// Its commits are on the base branch or the checkout's branch: deleting it loses nothing.
  pub merged: bool,
}

/// Everything the run left.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Leftovers {
  pub worktrees: Vec<LeftWorktree>,
  pub branches: Vec<LeftBranch>,
}

impl Leftovers {
  /// Files not committed in any of the worktrees.
  pub fn uncommitted(&self) -> usize {
    self.worktrees.iter().map(|worktree| worktree.uncommitted.len()).sum()
  }

  /// Whether the run's own worktree is still there. Its agent works in it when it is asked again,
  /// so once it was removed (or its folder deleted) no agent of the run can continue.
  pub fn keeps_run_worktree(&self) -> bool {
    self
      .worktrees
      .iter()
      .any(|worktree| worktree.owner == WorktreeOwner::Run && !worktree.missing)
  }
}

/// Why something was not cleared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeepReason {
  /// The worktree has uncommitted changes nobody confirmed discarding.
  Uncommitted(Vec<String>),
  /// The branch has commits the base branch does not.
  NotMerged,
  /// The branch is checked out in a worktree that was kept.
  InKeptWorktree,
  /// Git refused; its message.
  Failed(String),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CleanupError {
  /// Only a run that ended (succeeded, failed or cancelled) no longer works in its worktrees.
  #[error("the run has not ended")]
  NotFinished,
  #[error(transparent)]
  Git(#[from] GitError),
}

/// What clearing did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cleanup {
  pub removed: Vec<LeftWorktree>,
  pub deleted: Vec<BranchName>,
  pub kept_worktrees: Vec<(LeftWorktree, KeepReason)>,
  pub kept_branches: Vec<(BranchName, KeepReason)>,
}

/// What `run` left in the repository at `repo`, read with the `git` program.
pub fn leftovers(runner: &impl CommandRunner, git: &Path, repo: &Path, run: &Run) -> Result<Leftovers, GitError> {
  let git = Git::new(runner, git, repo);
  let listed = git.stdout(&["worktree", "list", "--porcelain", "-z"])?;
  let mut worktrees = Vec::new();
  for mut worktree in parse_worktrees(&listed)
    .into_iter()
    .filter_map(|listed| owned_by(run, listed))
  {
    if !worktree.missing {
      worktree.uncommitted = git.at(Path::new(&worktree.path)).uncommitted()?;
    }
    worktrees.push(worktree);
  }
  let targets = merge_targets(&git, run)?;
  let mut branches = Vec::new();
  for name in run_branches(&git, run)? {
    let mut merged = false;
    for target in &targets {
      merged = merged || git.is_ancestor(&format!("refs/heads/{name}"), target)?;
    }
    branches.push(LeftBranch { name, merged });
  }
  Ok(Leftovers { worktrees, branches })
}

/// Removes the worktrees of `run`, which ended, and deletes its merged branches. A worktree with
/// uncommitted changes is removed only if `confirmed`, what a person saw before confirming, lists
/// it with the same changes; otherwise it and its branch are kept. Git refusing one worktree or
/// branch keeps it with git's message and goes on with the others.
pub fn clean_up(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
  run: &Run,
  confirmed: Option<&Leftovers>,
) -> Result<Cleanup, CleanupError> {
  if !run.status().is_terminal() {
    return Err(CleanupError::NotFinished);
  }
  let current = leftovers(runner, git, repo, run)?;
  let git = Git::new(runner, git, repo);
  let mut cleanup = Cleanup::default();
  if current.worktrees.iter().any(|worktree| worktree.missing) {
    git.stdout(&["worktree", "prune"])?;
  }
  for worktree in current.worktrees {
    remove(&git, worktree, confirmed, &mut cleanup)?;
  }
  for branch in current.branches {
    let kept = cleanup
      .kept_worktrees
      .iter()
      .any(|(worktree, _)| worktree.branch.as_ref() == Some(&branch.name));
    let reason = match (branch.merged, kept) {
      (false, _) => Some(KeepReason::NotMerged),
      (true, true) => Some(KeepReason::InKeptWorktree),
      (true, false) => {
        let output = git.output(&["branch", "--quiet", "-D", branch.name.as_str()])?;
        (!output.success).then(|| KeepReason::Failed(super::git::message(&output)))
      }
    };
    match reason {
      Some(reason) => cleanup.kept_branches.push((branch.name, reason)),
      None => cleanup.deleted.push(branch.name),
    }
  }
  Ok(cleanup)
}

fn remove<R: CommandRunner>(
  git: &Git<'_, R>,
  worktree: LeftWorktree,
  confirmed: Option<&Leftovers>,
  cleanup: &mut Cleanup,
) -> Result<(), GitError> {
  if worktree.missing {
    cleanup.removed.push(worktree);
    return Ok(());
  }
  let discard = !worktree.uncommitted.is_empty();
  let agreed = confirmed.is_some_and(|confirmed| {
    confirmed
      .worktrees
      .iter()
      .any(|seen| seen.path == worktree.path && seen.uncommitted == worktree.uncommitted)
  });
  if discard && !agreed {
    let files = worktree.uncommitted.clone();
    cleanup.kept_worktrees.push((worktree, KeepReason::Uncommitted(files)));
    return Ok(());
  }
  let mut args = vec!["worktree", "remove"];
  if discard {
    args.push("--force");
  }
  args.push(&worktree.path);
  let output = git.output(&args)?;
  if output.success {
    cleanup.removed.push(worktree);
  } else {
    let reason = KeepReason::Failed(super::git::message(&output));
    cleanup.kept_worktrees.push((worktree, reason));
  }
  Ok(())
}

/// What a branch of the run must be reachable from to count as merged: the run's base branch and
/// the branch the checkout has now, as far as they exist. A detached `HEAD` does not count: commits
/// only it reaches are lost once it moves.
fn merge_targets<R: CommandRunner>(git: &Git<'_, R>, run: &Run) -> Result<Vec<String>, GitError> {
  let checked_out = git.checked_out_branch()?;
  let mut targets = Vec::new();
  for branch in run.base_branch().into_iter().chain(checked_out.as_ref()) {
    if let Some(commit) = git.commit(&format!("refs/heads/{branch}"))? {
      targets.push(commit);
    }
  }
  Ok(targets)
}

/// The run's branch and its step branches that exist.
fn run_branches<R: CommandRunner>(git: &Git<'_, R>, run: &Run) -> Result<Vec<BranchName>, GitError> {
  let own = format!("refs/heads/{}", run.branch());
  let steps = format!("{own}--*");
  let listed = git.stdout(&["for-each-ref", "--format=%(refname)", &own, &steps])?;
  listed
    .lines()
    .filter_map(|line| line.strip_prefix("refs/heads/"))
    .map(|name| {
      BranchName::new(name).map_err(|_| GitError::Unrecognized {
        expected: "a branch name",
        detail: name.to_owned(),
      })
    })
    .collect()
}

/// A worktree as `git worktree list` lists it.
struct Listed {
  path: String,
  branch: Option<BranchName>,
  prunable: bool,
}

/// `listed` as a worktree of `run`: its own, or one of its step executions', under
/// [`WORKTREES_DIR`].
fn owned_by(run: &Run, listed: Listed) -> Option<LeftWorktree> {
  let path = Path::new(&listed.path);
  let parent = path.parent()?;
  let in_worktrees = parent.file_name()? == "worktrees" && parent.parent()?.file_name()? == ".orchester";
  let name = path.file_name()?.to_str()?;
  if !in_worktrees {
    return None;
  }
  let owner = if name == run.id().to_string() {
    WorktreeOwner::Run
  } else {
    let execution = run
      .attempts()
      .iter()
      .flat_map(|attempt| attempt.executions())
      .find(|execution| execution.id().to_string() == name)?;
    WorktreeOwner::Step(execution.id())
  };
  Some(LeftWorktree {
    path: listed.path,
    owner,
    branch: listed.branch,
    uncommitted: Vec::new(),
    missing: listed.prunable,
  })
}

/// The worktrees `git worktree list --porcelain -z` lists: `worktree <path>`, then attributes
/// (`branch refs/heads/<name>`, `prunable …`), each ended by NUL, with an empty field between
/// worktrees.
fn parse_worktrees(listed: &str) -> Vec<Listed> {
  let mut worktrees: Vec<Listed> = Vec::new();
  for field in listed.split('\0') {
    if let Some(path) = field.strip_prefix("worktree ") {
      worktrees.push(Listed {
        path: path.to_owned(),
        branch: None,
        prunable: false,
      });
      continue;
    }
    let Some(current) = worktrees.last_mut() else {
      continue;
    };
    if let Some(branch) = field.strip_prefix("branch refs/heads/") {
      // Every branch git has passes the checks `BranchName` makes, which are a part of git's own.
      current.branch = BranchName::new(branch).ok();
    } else if field == "prunable" || field.starts_with("prunable ") {
      current.prunable = true;
    }
  }
  worktrees
}
