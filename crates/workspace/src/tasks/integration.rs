//! Integrating a task's latest run (02.13 `vpgrJ`): merging it into the branch the user's checkout
//! had when it started, recording the merge, and removing what the run left — its worktrees and
//! its merged branches — right after the merge, or for a run that ended once a person confirms it.
//! The rules are core's (`orchester_core::integration`); this reads and records what they need.
//! The codebase indexes only removed worktrees used are removed with them (`orchester_index`).
//! Blocks on SQLite and git; the UI calls it off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  command::CommandError,
  id::{RunId, StepExecutionId},
  integration::{
    Cleanup, CleanupError, FolderListing, GitError, KeepReason, LeftWorktree, Leftovers, MergeError,
    RecordedCleanupError, RunMerge, TemporaryLeft, clean_up_recorded_as_seen, leftovers_of, merge_run_at,
  },
  revision::Revision,
  run::{BranchName, Run},
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_index::CodebaseIndexes;
use orchester_store::{ProjectDb, StoreError};

use super::{changes::GIT, detail::run_store_error};
use crate::{
  activity::{Activity, Running, operations},
  messages::Message,
  workspace::OpenProject,
};

/// Names shown of a longer list of files, which then ends in an ellipsis.
const SHOWN_FILES: usize = 5;

/// Why integrating did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrationProblem {
  Merge(MergeError),
  Cleanup(CleanupError),
  Read(GitError),
  /// The project database could not be read or written.
  Store(Message),
  AlreadyMerged,
  /// Removing kept the worktree at `path`: changes appeared in it after the person saw it.
  NewChanges {
    path: String,
  },
  /// Nothing was removed: the run as stored now has other worktrees than the person saw (it worked again
  /// since), or is gone.
  RunChanged,
  /// Git did not remove the worktree at `path`; its message.
  NotRemoved {
    path: String,
    detail: String,
  },
  /// Git merged the run, but saving its receipt failed; external evidence prevents a repeated merge.
  NotRecorded {
    merge: RunMerge,
    problem: Message,
  },
}

/// Why a removal automation asked for did not happen: what a person reads, and whether the run changed since
/// automation read it ([`IntegrationProblem::RunChanged`]), which it names with a code of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovalRefused {
  pub message: Message,
  pub run_changed: bool,
}

impl From<Message> for RemovalRefused {
  fn from(message: Message) -> Self {
    Self {
      message,
      run_changed: false,
    }
  }
}

impl From<StoreError> for IntegrationProblem {
  fn from(error: StoreError) -> Self {
    tracing::error!(%error, "cannot read or record the run's integration");
    match error {
      StoreError::AlreadyMerged(_) => Self::AlreadyMerged,
      other => Self::Store(crate::messages::store_error(&other)),
    }
  }
}

impl IntegrationProblem {
  /// Ends the status bar's `running` operation for this problem. A refusal, a conflict or a
  /// worktree that stays ended the work undone, and the screen that asked says why: `not_done`,
  /// which the next event replaces (J). Work that could not run fails (H) with a short reason.
  pub fn end(&self, running: Running, not_done: Message) {
    let could_not_run = matches!(
      self,
      Self::Store(_)
        | Self::NotRecorded { .. }
        | Self::Read(_)
        | Self::Merge(MergeError::Git(_))
        | Self::Cleanup(CleanupError::Git(_))
    );
    if !could_not_run {
      running.finish_as(not_done);
      return;
    }
    let reason = match self {
      Self::Store(message) | Self::NotRecorded { problem: message, .. } => message.clone(),
      Self::Read(GitError::Run(CommandError::NotFound))
      | Self::Merge(MergeError::Git(GitError::Run(CommandError::NotFound)))
      | Self::Cleanup(CleanupError::Git(GitError::Run(CommandError::NotFound))) => {
        Message::new("runs.merge.failure.brief.gitMissing")
      }
      _ => Message::new("runs.merge.failure.brief.git"),
    };
    running.fail(reason);
  }

  /// What to tell the user; `separator` joins file names ("a.rs, b.rs").
  pub fn message(&self, separator: &str) -> Message {
    match self {
      Self::Merge(error) => merge_message(error, separator),
      Self::Cleanup(CleanupError::LiveWork) => Message::new("runs.worktree.failure.notFinished"),
      Self::Cleanup(CleanupError::LocalIntegration) => Message::new("runs.worktree.failure.localIntegration"),
      Self::Cleanup(CleanupError::Git(error)) | Self::Read(error) => git_message(error),
      Self::Store(message) => message.clone(),
      Self::AlreadyMerged => Message::new("runs.merge.failure.alreadyMerged"),
      Self::NewChanges { path } => Message::new("runs.worktree.failure.newChanges").with("path", short_path(path)),
      Self::RunChanged => Message::new("runs.worktree.failure.runChanged"),
      Self::NotRecorded { merge, .. } => {
        Message::new("runs.merge.failure.notRecorded").with("branch", merge.into.to_string())
      }
      Self::NotRemoved { path, detail } => Message::new("runs.worktree.failure.notRemoved")
        .with("path", short_path(path))
        .with("detail", short_reason(detail, path)),
    }
  }
}

fn merge_message(error: &MergeError, separator: &str) -> Message {
  let with_files = |key, files: &[String]| Message::new(key).with("files", file_list(files, separator));
  match error {
    MergeError::NotSucceeded => Message::new("runs.merge.failure.notSucceeded"),
    MergeError::NoBaseBranch => Message::new("runs.merge.failure.noBaseBranch"),
    MergeError::WrongBranch {
      expected,
      current: Some(current),
    } => Message::new("runs.merge.failure.wrongBranch")
      .with("branch", expected.to_string())
      .with("current", current.to_string()),
    MergeError::WrongBranch {
      expected,
      current: None,
    } => Message::new("runs.merge.failure.detached").with("branch", expected.to_string()),
    MergeError::UncommittedInTheWay { files } => with_files("runs.merge.failure.uncommitted", files),
    MergeError::InProgress => Message::new("runs.merge.failure.inProgress"),
    MergeError::BaseMoved { branch } => Message::new("runs.merge.failure.baseMoved").with("branch", branch.to_string()),
    MergeError::LeftChanged { files, detail } => {
      with_files("runs.merge.failure.leftChanged", files).with("detail", detail.clone())
    }
    MergeError::BranchMissing(branch) => {
      Message::new("runs.merge.failure.branchMissing").with("branch", branch.to_string())
    }
    MergeError::BranchMoved { branch, expected } => Message::new("runs.merge.failure.branchMoved")
      .with("branch", branch.to_string())
      .with("revision", crate::display::short_revision(expected.as_str()).to_owned()),
    MergeError::Conflict { files } => with_files("runs.merge.failure.conflict", files),
    MergeError::Refused(said) => Message::new("runs.merge.failure.refused").with("detail", said.clone()),
    MergeError::Git(error) => git_message(error),
  }
}

fn git_message(error: &GitError) -> Message {
  match error {
    GitError::Run(CommandError::NotFound) => Message::new("runs.changes.failure.gitMissing"),
    GitError::Run(_) | GitError::Unrecognized { .. } => Message::new("runs.merge.failure.gitStopped"),
    GitError::Failed(said) => Message::new("runs.merge.failure.git").with("detail", said.clone()),
  }
}

/// Why the listing of a worktree folder git forgot stopped at an entry that could not be read, for
/// its run's row and the removal's dialog; `None` when it was read.
pub fn unread_entry(worktree: &LeftWorktree) -> Option<Message> {
  let FolderListing::Unreadable { path, detail } = &worktree.listing else {
    return None;
  };
  Some(
    Message::new("runs.worktree.failure.folderUnreadable")
      .with("path", short_path(path))
      .with("detail", short_reason(detail, path)),
  )
}

/// Characters of git's reason shown under a row or in a dialog; the log keeps all of it.
const REASON_CHARS: usize = 160;

/// A worktree's path as the app names it elsewhere, from `.orchester/worktrees/` on.
fn short_path(path: &str) -> String {
  let path = path.replace('\\', "/");
  path
    .find(".orchester/")
    .map_or_else(|| path.clone(), |start| path[start..].to_owned())
}

/// Git's reason for not removing the worktree at `path`, on one line: its first line without
/// git's `error:` or `fatal:`, the path shortened as [`short_path`] does, at most [`REASON_CHARS`].
fn short_reason(detail: &str, path: &str) -> String {
  let line = detail
    .replace('\\', "/")
    .replace(&path.replace('\\', "/"), &short_path(path))
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or_default()
    .to_owned();
  let line = ["error: ", "fatal: "]
    .iter()
    .find_map(|prefix| line.strip_prefix(prefix))
    .unwrap_or(&line)
    .to_owned();
  // "failed to delete '<path>': Invalid argument": the message names the path already.
  let line = match line.rsplit_once(": ") {
    Some((before, reason)) if before.contains(&short_path(path)) => reason.to_owned(),
    _ => line,
  };
  if line.chars().count() <= REASON_CHARS {
    return line;
  }
  let mut short: String = line.chars().take(REASON_CHARS).collect();
  short.push('…');
  short
}

/// At most [`SHOWN_FILES`] of `files`, joined with `separator`.
pub fn file_list(files: &[String], separator: &str) -> String {
  let mut list = files
    .iter()
    .take(SHOWN_FILES)
    .cloned()
    .collect::<Vec<_>>()
    .join(separator);
  if files.len() > SHOWN_FILES {
    list.push('…');
  }
  list
}

/// A merged run and what clearing up after it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
  pub merge: RunMerge,
  /// The temporary worktree of the merge commit, when it could not be removed.
  pub temporary: Option<TemporaryLeft>,
  /// Clearing up can fail on its own; the merge stands either way.
  pub cleanup: Result<Cleanup, IntegrationProblem>,
}

impl Merged {
  /// Ends `running`, the merge of run `run` of `task` in `open`, as done, and records a temporary
  /// worktree that stayed beside it for the person to delete; the merge.
  pub fn finish(
    self,
    running: Running,
    activity: &Activity,
    (open, task, run): (&OpenProject, orchester_core::id::TaskId, (&str, u32)),
  ) -> RunMerge {
    running.finish();
    if let Some(left) = &self.temporary {
      let (project, title, target) = operations::run_merge_left(open, task, run, &left.path);
      activity.record(Some(project), title, target);
    }
    self.merge
  }
}

/// Merges run `run` of the project at `repo_root`, records the merge and removes what the run left
/// that is safe to remove: its clean worktrees and its merged branches. With `expected`, only while the run's
/// branch is at that commit (the one a person reviewed); a branch that moved since merges nothing.
pub fn merge_in(
  repo_root: &Path,
  programs: &Programs,
  (run, expected): (RunId, Option<&Revision>),
) -> Result<Merged, IntegrationProblem> {
  let _lease = orchester_exec::local_git::IntegrationLease::acquire(repo_root)
    .map_err(|_| IntegrationProblem::Store(Message::new("localIntegration.preflightFailed")))?;
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(repo_root)
    .map_err(|_| IntegrationProblem::Store(Message::new("localIntegration.preflightFailed")))?;
  let scratch = orchester_exec::local_git::prepare_scratch(repo_root, &[".orchester", "local-integration", "legacy"])
    .map_err(|_| IntegrationProblem::Store(Message::new("localIntegration.preflightFailed")))?;
  let mut git_owner = orchester_exec::local_git::LocalGit::new(repo_root, &scratch)
    .map_err(|_| IntegrationProblem::Store(Message::new("localIntegration.preflightFailed")))?;
  git_owner
    .own_metadata()
    .map_err(|_| IntegrationProblem::Store(Message::new("localIntegration.preflightFailed")))?;
  let mut db = ProjectDb::open(repo_root)?;
  // Runs are never deleted: a run the task page read is there.
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(IntegrationProblem::Store(Message::new("errors.storage")))?;
  if db.run_merge(run.id())?.is_some() {
    return Err(IntegrationProblem::AlreadyMerged);
  }
  let task = db.task(run.task())?.ok_or(StoreError::TaskNotFound(run.task()))?;
  if super::local_integration::managed_checkout(&db, run.id())? {
    return Err(IntegrationProblem::Store(Message::new(
      "localIntegration.personRequired",
    )));
  }
  if super::local_integration::external_integration(repo_root, run.id())
    .map_err(|_| IntegrationProblem::Store(Message::new("localIntegration.preflightFailed")))?
    .is_some()
  {
    return Err(IntegrationProblem::Store(Message::new(
      "localIntegration.externalResult",
    )));
  }
  let latest = db
    .runs_of_task(run.task())
    .map_err(run_store_error)?
    .iter()
    .filter_map(|listed| {
      listed
        .as_ref()
        .map_or_else(|bad| bad.number.map(|n| n.get()), |run| Some(run.number().get()))
    })
    .max();
  if latest != Some(run.number().get()) {
    return Err(IntegrationProblem::Store(Message::new("appMcp.errors.notLatestRun")));
  }
  let git = programs.git_work();
  let outcome = merge_run_at(
    &git,
    (Path::new(GIT), repo_root),
    (&run, task.key()),
    expected,
    Timestamp::now(),
  )
  .map_err(IntegrationProblem::Merge)?;
  if let Some(left) = &outcome.left {
    tracing::warn!(path = %left.path, detail = %left.detail, "merged, but the temporary worktree of the merge commit stayed");
  }
  let (merge, temporary) = (outcome.merge, outcome.left);
  if merge.kind == orchester_core::integration::MergeKind::AlreadyMerged {
    return Err(IntegrationProblem::Store(Message::new(
      "localIntegration.externalResult",
    )));
  }
  // An unrecorded completed merge remains external evidence on retry; ancestry cannot invent its historical event.
  if let Err(error) = db.save_run_merge(&merge) {
    return Err(match IntegrationProblem::from(error) {
      IntegrationProblem::Store(problem) => IntegrationProblem::NotRecorded { merge, problem },
      // Another window recorded the merge first: it stands as recorded.
      other => other,
    });
  }
  let cleanup = clear(&mut db, programs, repo_root, &run, None);
  if cleanup.is_err() {
    tracing::warn!("the run was merged, but what it left could not be cleared");
  }
  Ok(Merged {
    merge,
    temporary,
    cleanup,
  })
}

/// What the task's runs left in the repository, by run, as the task page holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftBehind(pub Vec<(RunId, Leftovers)>);

/// What each of `runs` left in the repository at `repo_root`, by run.
pub fn leftovers_in(
  repo_root: &Path,
  programs: &Programs,
  runs: &[Arc<Run>],
) -> Result<LeftBehind, IntegrationProblem> {
  let listed: Vec<&Run> = runs.iter().map(AsRef::as_ref).collect();
  leftovers_of(&programs.runner(), Path::new(GIT), repo_root, &listed)
    .map(|left| LeftBehind(runs.iter().map(|run| run.id()).zip(left).collect()))
    .map_err(|error| {
      tracing::warn!(%error, "cannot read what the runs left");
      IntegrationProblem::Read(error)
    })
}

/// Removes what `run` left, discarding uncommitted changes only where `confirmed`, what the person
/// saw, lists exactly those, and records when its own worktree went. A worktree that stays is a
/// problem to show: it changed since, or git refused. The run is read again first: one that has other
/// worktrees than `run`, the run the person saw, removes nothing ([`IntegrationProblem::RunChanged`]).
pub fn remove_in(
  repo_root: &Path,
  programs: &Programs,
  run: &Run,
  confirmed: &Leftovers,
) -> Result<(), IntegrationProblem> {
  let mut db = ProjectDb::open(repo_root)?;
  let stored = db
    .run(run.id())
    .map_err(run_store_error)?
    .filter(|stored| worktree_owners(stored) == worktree_owners(run))
    .ok_or(IntegrationProblem::RunChanged)?;
  let cleanup = clear(&mut db, programs, repo_root, &stored, Some(confirmed))?;
  match cleanup.kept_worktrees.into_iter().next() {
    None => Ok(()),
    Some((worktree, KeepReason::Failed(detail))) => Err(IntegrationProblem::NotRemoved {
      path: worktree.path,
      detail,
    }),
    Some((worktree, _)) => Err(IntegrationProblem::NewChanges { path: worktree.path }),
  }
}

/// What finds the worktrees and branches `run` left (`orchester_core::integration`): its branch and the
/// worktree of each of its step executions, beside its own.
fn worktree_owners(run: &Run) -> (&BranchName, Vec<StepExecutionId>) {
  let executions = run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .map(|execution| execution.id())
    .collect();
  (run.branch(), executions)
}

/// Removes what `run` left with the removal recorded first, refused once the run has live work as
/// stored now, e.g. continued from another window since its page was read, or, read again once the
/// removal is recorded, has other worktrees than `run` ([`clean_up_recorded_as_seen`]).
fn clear(
  db: &mut ProjectDb,
  programs: &Programs,
  repo_root: &Path,
  run: &Run,
  confirmed: Option<&Leftovers>,
) -> Result<Cleanup, IntegrationProblem> {
  let git = programs.git_work();
  let unchanged = |db: &ProjectDb| {
    let stored = db.run(run.id()).map_err(run_store_error)?;
    Ok(stored.is_some_and(|stored| worktree_owners(&stored) == worktree_owners(run)))
  };
  let git = (&git, Path::new(GIT), repo_root);
  let cleanup = clean_up_recorded_as_seen(db, git, run, confirmed, (Timestamp::now(), unchanged)).map_err(|error| {
    tracing::warn!("cannot remove what the run left");
    match error {
      RecordedCleanupError::Cleanup(error) => IntegrationProblem::Cleanup(error),
      RecordedCleanupError::Store(error) => IntegrationProblem::from(error),
      RecordedCleanupError::RunChanged => IntegrationProblem::RunChanged,
    }
  })?;
  prune_indexes(repo_root);
  Ok(cleanup)
}

/// Removes the codebase indexes that only removed worktrees used. The worktrees are gone either way;
/// an index left behind goes with the next removal.
fn prune_indexes(repo_root: &Path) {
  if let Err(error) = CodebaseIndexes::new(repo_root).prune() {
    tracing::warn!(%error, "cannot remove the codebase indexes of removed worktrees");
  }
}
