//! Integrating a task's latest run (02.13 `vpgrJ`): merging it into the base branch it recorded
//! when it started (in the user's checkout, or beside it when the project's base branch is set and
//! the checkout has another branch), recording the merge, and removing what the run left — its
//! worktrees and its merged branches — right after the merge, or for a run that ended once a person
//! confirms it. The rules are core's (`orchester_core::integration`); this reads and records what
//! they need. The codebase indexes only removed worktrees used are removed with them
//! (`orchester_index`). Blocks on SQLite and git; the UI calls it off the UI thread.

mod preflight;
mod words;

use std::{path::Path, sync::Arc};

use orchester_core::{
  command::CommandError,
  id::{RunId, StepExecutionId},
  integration::{
    Cleanup, CleanupError, Delivering, GitError, KeepReason, Leftovers, MergeError, PreflightCause, PreflightStage,
    RecordedCleanupError, RunMerge, TemporaryLeft,
    attempt::{AttemptStage, MergeAsker, MergeVia},
    base::Landing,
    leftovers_of, merge_run_as, record_removal_as_seen, record_what_stayed,
    removal::{Discard, RemovalRequest},
    settle_removal,
  },
  revision::Revision,
  run::{BranchName, Run},
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::{
  local_git::{IntegrationLease, LocalGit, LocalGitError, owned::StoreOwnership},
  programs::Programs,
};
use orchester_store::{ProjectDb, StoreError};
pub use preflight::{cause_words, stage_message};
pub(crate) use preflight::{hold, integration_refusal};
pub(crate) use words::git_message;
pub use words::{file_list, unread_entry};
use words::{merge_message, short_path, short_reason};

use super::{
  changes::GIT,
  detail::run_store_error,
  local_integration::CheckoutHolder,
  merge_attempts::{Asked, Attempt},
  run::control_message,
  worktree_removal::{Unremoved, WorktreeRemover},
};
use crate::{
  activity::{Activity, Running, run_operations},
  messages::Message,
  runner::RunRequestError,
  workspace::OpenProject,
};

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
  /// A local integration holds the checkout with work it must finish, put back or clean up first: this run's, or
  /// another task's. Nothing was merged.
  HeldLocally(CheckoutHolder),
  /// Git merged the run, but saving its receipt failed; external evidence prevents a repeated merge.
  NotRecorded {
    merge: RunMerge,
    problem: Message,
  },
  /// The project's runner, which removes runs' worktrees, did not carry the removal out: nothing was removed.
  Runner(RunRequestError),
  /// The merge's preflight refused at `stage` before Git ran: nothing was merged, and the branch and checkout are
  /// unchanged.
  Preflight {
    stage: PreflightStage,
    cause: PreflightCause,
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
        | Self::Preflight { .. }
        | Self::Read(_)
        | Self::Merge(MergeError::Git(_) | MergeError::Unanswered { .. })
        | Self::Cleanup(CleanupError::Git(_))
        | Self::Runner(_)
    );
    if !could_not_run {
      running.finish_as(not_done);
      return;
    }
    let reason = match self {
      Self::Store(message) | Self::NotRecorded { problem: message, .. } => message.clone(),
      Self::Preflight { stage, cause } => preflight::message(*stage, cause),
      Self::Read(GitError::Run(CommandError::NotFound))
      | Self::Merge(MergeError::Git(GitError::Run(CommandError::NotFound)))
      | Self::Cleanup(CleanupError::Git(GitError::Run(CommandError::NotFound))) => {
        Message::new("runs.merge.failure.brief.gitMissing")
      }
      Self::Runner(error) => control_message(error),
      _ => Message::new("runs.merge.failure.brief.git"),
    };
    running.fail(reason);
  }

  /// Whether Local integration carries on where the direct merge stopped: it commits or preserves uncommitted
  /// files the merge would overwrite, resolves a conflict in the run's worktree, or holds the checkout for this
  /// run's own review already. A checkout on another branch, a git operation in progress or another run's local
  /// integration are not its to settle.
  pub fn continues_locally(&self) -> bool {
    matches!(
      self,
      Self::Merge(MergeError::UncommittedInTheWay { .. } | MergeError::Conflict { .. })
        | Self::HeldLocally(CheckoutHolder::ThisRun)
    )
  }

  /// Whether the merge stopped for a cause that passes by itself, which Orchester tries again
  /// ([`orchester_core::integration::retry`]): the project's integration lease another operation holds, a file
  /// another program holds open, the project database busy, a Git program of the run that is still closing.
  pub fn passes(&self) -> bool {
    match self {
      Self::Preflight { cause, .. } => cause.passes(),
      Self::Merge(error) => error.passes(),
      Self::Store(message) => crate::messages::failure_kind(message) == crate::messages::FailureKind::Busy,
      Self::HeldLocally(CheckoutHolder::RunningProgram) => true,
      _ => false,
    }
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
      Self::HeldLocally(CheckoutHolder::ThisRun) => Message::new("runs.merge.failure.heldLocally"),
      Self::HeldLocally(CheckoutHolder::RunningProgram) => Message::new("runs.merge.failure.programRunning"),
      Self::HeldLocally(CheckoutHolder::OtherRun { task }) => {
        Message::new("runs.merge.failure.heldByOther").with("task", task.to_string())
      }
      Self::NewChanges { path } => Message::new("runs.worktree.failure.newChanges").with("path", short_path(path)),
      Self::RunChanged => Message::new("runs.worktree.failure.runChanged"),
      Self::NotRecorded { merge, .. } => {
        Message::new("runs.merge.failure.notRecorded").with("branch", merge.into.to_string())
      }
      Self::NotRemoved { path, detail } => Message::new("runs.worktree.failure.notRemoved")
        .with("path", short_path(path))
        .with("detail", short_reason(detail, path)),
      Self::Runner(error) => control_message(error),
      Self::Preflight { stage, cause } => preflight::message(*stage, cause),
    }
  }
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
      let (project, title, target) = run_operations::run_merge_left(open, task, run, &left.path);
      activity.record(Some(project), title, target);
    }
    self.merge
  }
}

/// Refuses a run merged already, or one a merge waiting in its worktree for the person holds back: before git changes
/// anything, as the record of the merge would be refused after it.
fn not_merged_or_held(db: &ProjectDb, run: RunId) -> Result<(), IntegrationProblem> {
  if db.run_merge(run)?.is_some() {
    return Err(IntegrationProblem::AlreadyMerged);
  }
  if db.resolution(run)?.is_some() {
    return Err(IntegrationProblem::Store(Message::new(
      "runs.merge.failure.notMergeable",
    )));
  }
  Ok(())
}

/// Merges run `run` of the project whose run worktrees are `worktrees` as a person asks on its task page, with
/// `programs` running git in the person's checkout, records the merge and has the project's runner remove what the
/// run left that is safe to remove: its clean worktrees and its merged branches. With `expected`, only while the run's
/// branch is at that commit (the one a person reviewed); a branch that moved since merges nothing.
pub fn merge_in(
  worktrees: &WorktreeRemover,
  programs: &Programs,
  (run, expected): (RunId, Option<&Revision>),
) -> Result<Merged, IntegrationProblem> {
  merge_in_as(
    worktrees,
    programs,
    (run, expected, None),
    (MergeAsker::Person, MergeVia::TaskPage),
  )
}

/// What a module delivery's merge into the person's repository holds while git changes it: the project's integration
/// lease, so no local integration or other merge works in it meanwhile, the ownership of its store and of its Git
/// metadata. A run's merge holds the same through its preflight ([`preflight::hold`]), which tells each refusal's
/// stage.
pub(crate) struct MergeHold {
  _lease: IntegrationLease,
  _store_owner: Arc<StoreOwnership>,
  _git_owner: LocalGit,
}

impl MergeHold {
  /// Holds the repository at `repo_root`; refused while another merge or a local integration holds it.
  pub(crate) fn acquire(repo_root: &Path) -> Result<Self, LocalGitError> {
    let lease = IntegrationLease::acquire(repo_root)?;
    let store_owner = StoreOwnership::acquire(repo_root)?;
    let scratch =
      orchester_exec::local_git::prepare_scratch(repo_root, &[".orchester", "local-integration", "legacy"])?;
    let mut git_owner = LocalGit::new(repo_root, &scratch)?;
    git_owner.own_metadata()?;
    Ok(Self {
      _lease: lease,
      _store_owner: store_owner,
      _git_owner: git_owner,
    })
  }
}

/// [`merge_in`] as `asked` asks for it, landed as `delivering` says when the merge queue delivers the run (`None`: as
/// the task page merges). The attempt is recorded however it ends ([`Attempt`]).
pub fn merge_in_as(
  worktrees: &WorktreeRemover,
  programs: &Programs,
  (run, expected, delivering): (RunId, Option<&Revision>, Option<Delivering<'_>>),
  asked: Asked,
) -> Result<Merged, IntegrationProblem> {
  let repo_root = worktrees.root();
  // Held until the attempt is recorded, so that no other merge takes its open attempt for one the app left.
  let held = preflight::hold(repo_root);
  let mut attempt = Attempt::begin((repo_root, programs), run, asked);
  let merged = match &held {
    Ok(_) => merge_held(worktrees, programs, (run, expected, delivering), &mut attempt),
    Err(problem) => Err(problem.clone()),
  };
  attempt.end(&merged);
  merged
}

/// [`merge_in_as`] once its preflight holds the checkout, the stages it reaches told to `attempt`.
fn merge_held(
  worktrees: &WorktreeRemover,
  programs: &Programs,
  (run, expected, delivering): (RunId, Option<&Revision>, Option<Delivering<'_>>),
  attempt: &mut Attempt<'_>,
) -> Result<Merged, IntegrationProblem> {
  let repo_root = worktrees.root();
  let mut db = ProjectDb::open(repo_root)?;
  attempt.reach(&mut db, AttemptStage::ExistingResult);
  // Runs are never deleted: a run the task page read is there.
  let run = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(IntegrationProblem::Store(Message::new("errors.storage")))?;
  let task = mergeable(repo_root, &db, &run)?;
  attempt.reach(&mut db, AttemptStage::Merge);
  let git = programs.merge_git();
  let landing = Landing::of(run.base_source());
  let outcome = merge_run_as(
    &git,
    (Path::new(GIT), repo_root),
    (&run, task.key()),
    (expected, delivering, landing),
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
  attempt.reach(&mut db, AttemptStage::Record);
  // An unrecorded completed merge remains external evidence on retry; ancestry cannot invent its historical event.
  if let Err(error) = db.save_run_merge(&merge) {
    return Err(match IntegrationProblem::from(error) {
      IntegrationProblem::Store(problem) => IntegrationProblem::NotRecorded { merge, problem },
      // Another window recorded the merge first: it stands as recorded.
      other => other,
    });
  }
  super::local_integration::settle_review(&mut db, &merge);
  crate::base_moved::after_merge(repo_root, programs);
  let cleanup = clear(&mut db, worktrees, &run, None, false);
  if cleanup.is_err() {
    tracing::warn!("the run was merged, but what it left could not be cleared");
  }
  Ok(Merged {
    merge,
    temporary,
    cleanup,
  })
}

/// Refuses `run` unless it may merge: not merged already nor held back by a merge waiting in its worktree, no local
/// integration holding the checkout, its result not in Git already, and its task's latest run; its task.
fn mergeable(repo_root: &Path, db: &ProjectDb, run: &Run) -> Result<orchester_core::task::Task, IntegrationProblem> {
  not_merged_or_held(db, run.id())?;
  // A team task's run goes into its module's branch and only the module's delivery takes it further: it is never merged
  // into the base branch from here, whoever asks.
  if db.team_run(run.id())?.is_some() {
    return Err(IntegrationProblem::Store(Message::new("localIntegration.teamRun")));
  }
  let task = db.task(run.task())?.ok_or(StoreError::TaskNotFound(run.task()))?;
  if let Some(holder) = super::local_integration::checkout_holder(db, run.id())? {
    return Err(IntegrationProblem::HeldLocally(holder));
  }
  if super::local_integration::external_integration(repo_root, run.id())
    .map_err(integration_refusal(repo_root, PreflightStage::ExistingResult))?
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
  Ok(task)
}

/// What the task's runs left in the repository, by run, as the task page holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftBehind(pub Vec<(RunId, Leftovers)>);

/// What each of `runs` left in the repository at `repo_root`, by run, with whether each borrows workspaces as its
/// project's store says.
pub fn leftovers_in(
  repo_root: &Path,
  programs: &Programs,
  runs: &[Arc<Run>],
) -> Result<LeftBehind, IntegrationProblem> {
  use orchester_core::store::WorkspaceStore;
  let listed: Vec<&Run> = runs.iter().map(AsRef::as_ref).collect();
  let mut left = leftovers_of(&programs.runner(), Path::new(GIT), repo_root, &listed).map_err(|error| {
    tracing::warn!(%error, "cannot read what the runs left");
    IntegrationProblem::Read(error)
  })?;
  let db = ProjectDb::open(repo_root)?;
  for (run, left) in runs.iter().zip(&mut left) {
    let has_branch = left.branches.iter().any(|branch| branch.name == *run.branch());
    left.borrows = has_branch && db.run_borrows(run.id())?;
  }
  Ok(LeftBehind(runs.iter().map(|run| run.id()).zip(left).collect()))
}

/// Removes what `run` left, among `worktrees`, discarding uncommitted changes only where `confirmed`, what the person
/// saw, lists exactly those, and records when its own worktree went. A worktree that stays is a
/// problem to show: it changed since, or git refused. The run is read again first: one that has other
/// worktrees than `run`, the run the person saw, removes nothing ([`IntegrationProblem::RunChanged`]).
pub fn remove_in(worktrees: &WorktreeRemover, run: &Run, confirmed: &Leftovers) -> Result<(), IntegrationProblem> {
  remove_as(worktrees, run, confirmed, false)
}

/// Storage cleanup keeps all branches and refuses runs that have not ended.
pub fn remove_storage_in(
  worktrees: &WorktreeRemover,
  run: &Run,
  confirmed: &Leftovers,
) -> Result<(), IntegrationProblem> {
  if !run.status().is_terminal() {
    return Err(IntegrationProblem::Cleanup(CleanupError::LiveWork));
  }
  remove_as(worktrees, run, confirmed, true)
}

fn remove_as(
  worktrees: &WorktreeRemover,
  run: &Run,
  confirmed: &Leftovers,
  preserve_branches: bool,
) -> Result<(), IntegrationProblem> {
  let mut db = ProjectDb::open(worktrees.root())?;
  let stored = db
    .run(run.id())
    .map_err(run_store_error)?
    .filter(|stored| worktree_owners(stored) == worktree_owners(run))
    .ok_or(IntegrationProblem::RunChanged)?;
  let cleanup = clear(&mut db, worktrees, &stored, Some(confirmed), preserve_branches)?;
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
/// removal is recorded, has other worktrees than `run` ([`record_removal_as_seen`]). The project's runner removes
/// (`worktrees`): a folder something held is deleted again once the run's own processes there are stopped, then the
/// codebase indexes only removed worktrees used go and folders no run uses any more are swept. The record is taken
/// back when nothing was touched ([`settle_removal`]), and a folder that still stays is recorded as not removed
/// ([`record_what_stayed`]). A Storage clean-up (`preserve_branches`) keeps every branch, refuses a run that has not
/// ended and sweeps no folders ([`Discard::Worktrees`]).
pub(super) fn clear(
  db: &mut ProjectDb,
  worktrees: &WorktreeRemover,
  run: &Run,
  confirmed: Option<&Leftovers>,
  preserve_branches: bool,
) -> Result<Cleanup, IntegrationProblem> {
  let unchanged = |db: &ProjectDb| {
    let stored = db.run(run.id()).map_err(run_store_error)?;
    Ok(stored.is_some_and(|stored| {
      worktree_owners(&stored) == worktree_owners(run) && (!preserve_branches || stored.status().is_terminal())
    }))
  };
  let at = Timestamp::now();
  let start = record_removal_as_seen(db, run.id(), (at, unchanged)).map_err(|error| {
    tracing::warn!("cannot remove what the run left");
    match error {
      RecordedCleanupError::Cleanup(error) => IntegrationProblem::Cleanup(error),
      RecordedCleanupError::Store(error) => IntegrationProblem::from(error),
      RecordedCleanupError::RunChanged => IntegrationProblem::RunChanged,
    }
  })?;
  let confirmed = confirmed.cloned();
  let request = RemovalRequest {
    discard: if preserve_branches {
      Discard::Worktrees(confirmed)
    } else {
      Discard::Seen(confirmed)
    },
    retry_held: true,
  };
  let removed = worktrees.remove(run.id(), request);
  // Removing fails only before it removes anything: nothing was touched.
  settle_removal(db, (run.id(), start, at), removed.as_ref().ok())?;
  let cleanup = removed.map_err(|unremoved| {
    tracing::warn!(%unremoved, "cannot remove what the run left");
    IntegrationProblem::from(unremoved)
  })?;
  if let Err(error) = record_what_stayed(db, (run.id(), Timestamp::now()), &cleanup) {
    // The worktrees are as the clean-up says either way; only the record of what stayed is missing.
    tracing::warn!(%error, run = %run.id(), "cannot record what the worktree removal left");
  }
  if let Some(left) = cleanup.not_removed() {
    tracing::warn!(run = %run.id(), path = %left.path, reason = ?left.reason, "a worktree of the run was not removed");
  }
  Ok(cleanup)
}

impl From<Unremoved> for IntegrationProblem {
  fn from(unremoved: Unremoved) -> Self {
    match unremoved {
      Unremoved::Runner(error) => Self::Runner(error),
      Unremoved::Failed(failure) => match failure.cleanup_error() {
        Some(error) => Self::Cleanup(error),
        None => Self::Store(Message::new("errors.storage")),
      },
    }
  }
}
