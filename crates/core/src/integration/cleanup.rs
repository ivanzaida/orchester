//! What a run left in the repository, and clearing it (`docs/architecture/storage.md`, "Agent
//! worktrees"): the run's worktree and the worktrees of steps that ran beside others and were kept
//! (a step that failed or was skipped keeps its own), under `.orchester/worktrees/`, and the run's
//! branch and step branches. A worktree with uncommitted changes is removed only when a person saw
//! those very changes and confirmed discarding them; a branch is deleted only when its commits are
//! on the run's base branch or the branch the checkout has now, so nothing committed is lost.
//! Every other branch is kept.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{FolderListing, GitError, git::Git};
use crate::{
  command::CommandRunner,
  id::StepExecutionId,
  run::{BranchName, Run},
};

/// Where runs' worktrees are, relative to the repository.
pub const WORKTREES_DIR: &str = ".orchester/worktrees";

/// Whose worktree it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "execution", rename_all = "snake_case")]
pub enum WorktreeOwner {
  Run,
  /// A step execution that ran beside other steps of the run.
  Step(StepExecutionId),
}

/// A worktree of the run, as git lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
  /// Git no longer lists it, but its folder is there: a removal that failed part way left it
  /// ([`super::forgotten`]). Its files are listed in `uncommitted`; removing it deletes the folder.
  #[serde(default)]
  pub forgotten: bool,
  /// How much of a forgotten folder's files `uncommitted` lists.
  #[serde(default)]
  pub listing: FolderListing,
}

impl LeftWorktree {
  /// Whether removing it loses files, which a person must have agreed to: uncommitted changes, or
  /// a forgotten folder that holds more than it lists.
  pub fn loses_files(&self) -> bool {
    !self.uncommitted.is_empty() || self.listing != FolderListing::Whole
  }
}

/// A branch of the run: its own or a step's (`<run-branch>--<step>-<attempt>`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftBranch {
  pub name: BranchName,
  /// Its commits are on the base branch or the checkout's branch: deleting it loses nothing.
  pub merged: bool,
}

/// Everything the run left. It travels to the runner as what a person saw before confirming its
/// removal (Run again).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leftovers {
  pub worktrees: Vec<LeftWorktree>,
  pub branches: Vec<LeftBranch>,
}

impl Leftovers {
  /// Files not committed in any of the worktrees, as far as they were listed.
  pub fn uncommitted(&self) -> usize {
    self.worktrees.iter().map(|worktree| worktree.uncommitted.len()).sum()
  }

  /// Whether removing them loses files ([`LeftWorktree::loses_files`]).
  pub fn loses_files(&self) -> bool {
    self.worktrees.iter().any(LeftWorktree::loses_files)
  }

  /// Whether a forgotten folder among them holds more files than are listed: reading it stopped
  /// after its first entries.
  pub fn listing_cut(&self) -> bool {
    self
      .worktrees
      .iter()
      .any(|worktree| worktree.listing == FolderListing::Cut)
  }

  /// Whether reading a forgotten folder among them stopped at an entry that could not be read.
  pub fn listing_unreadable(&self) -> bool {
    self
      .worktrees
      .iter()
      .any(|worktree| matches!(worktree.listing, FolderListing::Unreadable { .. }))
  }

  /// Whether the run's own worktree is still there. Its agent works in it when it is asked again,
  /// so once it was removed (or its folder deleted) no agent of the run can continue.
  pub fn keeps_run_worktree(&self) -> bool {
    self
      .worktrees
      .iter()
      .any(|worktree| worktree.owner == WorktreeOwner::Run && !worktree.missing && !worktree.forgotten)
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
  /// The run still works in its worktrees: only a run that ended (succeeded, failed or
  /// cancelled) or was interrupted no longer does.
  #[error("the run still works in its worktrees")]
  LiveWork,
  /// Local integration still owns the run's worktree or needs it for recovery.
  #[error("local integration must be released before removing its worktrees")]
  LocalIntegration,
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

impl Cleanup {
  /// Whether the run's own worktree went (or only its folder was gone and git forgot it): its agent
  /// can never continue in it, which is worth recording.
  pub fn removed_run_worktree(&self) -> bool {
    self.removed.iter().any(|worktree| worktree.owner == WorktreeOwner::Run)
  }
}

/// What `run` left in the repository at `repo`, read with the `git` program.
pub fn leftovers(runner: &impl CommandRunner, git: &Path, repo: &Path, run: &Run) -> Result<Leftovers, GitError> {
  let mut left = leftovers_of(runner, git, repo, &[run])?;
  Ok(left.pop().expect("leftovers_of reads what each run left"))
}

/// What each of `runs` left in the repository at `repo`, in their order: git lists the worktrees
/// and reads the checkout's branch once for all of them.
pub fn leftovers_of(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
  runs: &[&Run],
) -> Result<Vec<Leftovers>, GitError> {
  let git = Git::new(runner, git, repo);
  let listed = parse_worktree_list(&git.stdout(&["worktree", "list", "--porcelain", "-z"])?);
  let checked_out = git.checked_out_branch()?;
  let mut all = Vec::with_capacity(runs.len());
  for run in runs {
    let mut worktrees = Vec::new();
    for mut worktree in listed.iter().filter_map(|listed| owned_by(run, listed)) {
      if !worktree.missing {
        worktree.uncommitted = git.at(Path::new(&worktree.path)).uncommitted()?;
      }
      worktrees.push(worktree);
    }
    let forgotten = super::forgotten::forgotten(repo, run, &worktrees);
    worktrees.extend(forgotten);
    let targets = merge_targets(&git, run, checked_out.as_ref())?;
    let mut branches = Vec::new();
    for name in run_branches(&git, run)? {
      let mut merged = false;
      for target in &targets {
        merged = merged || git.is_ancestor(&format!("refs/heads/{name}"), target)?;
      }
      branches.push(LeftBranch { name, merged });
    }
    all.push(Leftovers { worktrees, branches });
  }
  Ok(all)
}

/// Removes the worktrees of `run`, which ended, and deletes its merged branches. A worktree with
/// uncommitted changes (or a forgotten folder with files) is removed only if `confirmed`, what a
/// person saw before confirming, lists it with the same changes, read as far; otherwise it and its branch are
/// kept. An error comes only before anything is removed; once removing started, git refusing or not
/// finishing one worktree or branch keeps it with why and goes on with the others.
pub fn clean_up(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
  run: &Run,
  confirmed: Option<&Leftovers>,
) -> Result<Cleanup, CleanupError> {
  clear(runner, (git, repo), run, Discard::Seen(confirmed))
}

/// Removes every worktree of `run`, which ended, uncommitted changes and all, and deletes every branch of it, merged
/// or not: what a person who deletes the run's task chose to lose with it (ORC-91). Commits merged elsewhere stay
/// on the branches they were merged into. Errors and refusals come as [`clean_up`]'s do.
pub fn clean_up_all(runner: &impl CommandRunner, git: &Path, repo: &Path, run: &Run) -> Result<Cleanup, CleanupError> {
  clear(runner, (git, repo), run, Discard::Everything)
}

/// What a clean-up may lose: the uncommitted changes a person saw and nothing unmerged, or everything the run left.
#[derive(Clone, Copy)]
enum Discard<'a> {
  Seen(Option<&'a Leftovers>),
  Everything,
}

fn clear(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  run: &Run,
  discard: Discard<'_>,
) -> Result<Cleanup, CleanupError> {
  if run.status().has_live_work() {
    return Err(CleanupError::LiveWork);
  }
  let current = leftovers(runner, git, repo, run)?;
  let git = Git::new(runner, git, repo);
  let mut cleanup = Cleanup::default();
  if current.worktrees.iter().any(|worktree| worktree.missing) {
    git.stdout(&["worktree", "prune"])?;
  }
  for worktree in current.worktrees {
    remove(&git, worktree, discard, &mut cleanup);
  }
  let unmerged_too = matches!(discard, Discard::Everything);
  for branch in current.branches {
    let kept = cleanup
      .kept_worktrees
      .iter()
      .any(|(worktree, _)| worktree.branch.as_ref() == Some(&branch.name));
    let reason = match (branch.merged || unmerged_too, kept) {
      (false, _) => Some(KeepReason::NotMerged),
      (true, true) => Some(KeepReason::InKeptWorktree),
      (true, false) => match git.output(&["branch", "--quiet", "-D", branch.name.as_str()]) {
        Ok(output) => (!output.success()).then(|| KeepReason::Failed(super::git::message(&output))),
        Err(error) => Some(KeepReason::Failed(error.to_string())),
      },
    };
    match reason {
      Some(reason) => cleanup.kept_branches.push((branch.name, reason)),
      None => cleanup.deleted.push(branch.name),
    }
  }
  Ok(cleanup)
}

fn remove<R: CommandRunner>(git: &Git<'_, R>, worktree: LeftWorktree, discard: Discard<'_>, cleanup: &mut Cleanup) {
  if worktree.missing {
    cleanup.removed.push(worktree);
    return;
  }
  let agreed = match discard {
    Discard::Everything => true,
    Discard::Seen(confirmed) => confirmed.is_some_and(|confirmed| {
      confirmed.worktrees.iter().any(|seen| {
        seen.path == worktree.path && seen.uncommitted == worktree.uncommitted && seen.listing == worktree.listing
      })
    }),
  };
  let discard = worktree.loses_files();
  if discard && !agreed {
    let files = worktree.uncommitted.clone();
    cleanup.kept_worktrees.push((worktree, KeepReason::Uncommitted(files)));
    return;
  }
  let outcome = if worktree.forgotten {
    super::forgotten::delete(&worktree)
  } else {
    let mut args = vec!["worktree", "remove"];
    if discard {
      args.push("--force");
    }
    args.push(&worktree.path);
    match git.output(&args) {
      Ok(output) if output.success() => Ok(()),
      Ok(output) => Err(super::git::message(&output)),
      Err(error) => Err(error.to_string()),
    }
  };
  match outcome {
    Ok(()) => cleanup.removed.push(worktree),
    Err(detail) => cleanup.kept_worktrees.push((worktree, KeepReason::Failed(detail))),
  }
}

/// What a branch of the run must be reachable from to count as merged: the run's base branch and
/// the branch the checkout has now, as far as they exist. A detached `HEAD` does not count: commits
/// only it reaches are lost once it moves.
fn merge_targets<R: CommandRunner>(
  git: &Git<'_, R>,
  run: &Run,
  checked_out: Option<&BranchName>,
) -> Result<Vec<String>, GitError> {
  let mut targets = Vec::new();
  for branch in run.base_branch().into_iter().chain(checked_out) {
    if let Some(commit) = git.commit(&format!("refs/heads/{branch}"))? {
      targets.push(commit);
    }
  }
  Ok(targets)
}

/// The run's branch and its step branches that exist; none for a run that never started ([`Run::own_branch`]).
fn run_branches<R: CommandRunner>(git: &Git<'_, R>, run: &Run) -> Result<Vec<BranchName>, GitError> {
  let Some(branch) = run.own_branch() else {
    return Ok(Vec::new());
  };
  let own = format!("refs/heads/{branch}");
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

/// A worktree as `git worktree list --porcelain -z` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedWorktree {
  /// As git prints it: absolute, with `/` separators on every platform.
  pub path: String,
  /// The branch checked out there; `None` on a detached HEAD.
  pub branch: Option<BranchName>,
  /// Its folder is gone; git lists it until it is pruned.
  pub prunable: bool,
}

/// `listed` as a worktree of `run`: its own, or one of its step executions', under
/// [`WORKTREES_DIR`].
fn owned_by(run: &Run, listed: &ListedWorktree) -> Option<LeftWorktree> {
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
    path: listed.path.clone(),
    owner,
    branch: listed.branch.clone(),
    uncommitted: Vec::new(),
    missing: listed.prunable,
    forgotten: false,
    listing: FolderListing::Whole,
  })
}

/// The worktrees `git worktree list --porcelain -z` lists: `worktree <path>`, then attributes
/// (`branch refs/heads/<name>`, `prunable …`), each ended by NUL, with an empty field between
/// worktrees.
pub fn parse_worktree_list(listed: &str) -> Vec<ListedWorktree> {
  let mut worktrees: Vec<ListedWorktree> = Vec::new();
  for field in listed.split('\0') {
    if let Some(path) = field.strip_prefix("worktree ") {
      worktrees.push(ListedWorktree {
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
