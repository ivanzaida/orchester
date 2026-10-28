//! Turning a queued task into a started run: preflight, the run's base revision and its base branch (which merging
//! the run brings its work into: the project's base branch, or the one the user's checkout has), the run's branch,
//! its record in the database, its worktree and its start. Idempotent by the request's key.

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, StepExecutionId, TaskId},
  integration::{
    base::{BaseBranchSetting, RunBaseError, set_branch_tip},
    checked_out_branch,
  },
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, Tool, Version},
  revision::Revision,
  run::{
    BranchName, NewRun, PreflightInput, PreflightProblem, PublishedVersions, Run, RunFailure, RunSnapshot, RunStatus,
    WorktreeFailure, preflight,
  },
  sharing::TaskRevision,
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore, WorkspaceStore, WorktreeStore},
  task::{Task, TaskStatus},
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::{failed, read_failed};
use crate::{git::Git, worktree::Worktrees};

/// A git query that does not answer within this time counts as failed.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// A recorded run and whether this request is the one that executes it.
pub(super) struct Prepared {
  pub(super) run: Run,
  pub(super) task: Task,
  pub(super) folder: RunFolder,
  /// `false` when an earlier request with the same key already started the run.
  pub(super) execute: bool,
  /// A step a person asked to start on its account now, though the account's last reading says its plan limit
  /// holds: it starts once, and stops again if the CLI refuses.
  pub(super) try_at_plan_limit: Option<StepExecutionId>,
}

/// Where a run works.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RunFolder {
  /// Its own worktree, `.orchester/worktrees/<run>`, made before workspaces (ORC-114): it works there until it ends.
  Own(PathBuf),
  /// A warm workspace it borrows when its next step starts and gives back when it waits or ends.
  Borrowed,
}

pub(super) fn prepare_first(
  db: &mut ProjectDb,
  root: &Path,
  key: IdempotencyKey,
  task_id: TaskId,
) -> Result<Prepared, Rejection> {
  let checked = check(db, root, task_id, PipelineSource::QueuedPin)?;
  record(db, root, key, checked, None)
}

/// A queued task that passed the preflight, and the run it starts.
pub(super) struct Checked {
  task: Task,
  new: NewRun,
}

impl Checked {
  /// The task as it was read for the run.
  pub(super) const fn task(&self) -> &Task {
    &self.task
  }
}

/// Run again takes the latest published pipeline; the queued first run takes its saved version.
pub(super) fn check_again(db: &ProjectDb, root: &Path, task_id: TaskId) -> Result<Checked, Rejection> {
  check(db, root, task_id, PipelineSource::LatestPublished)
}

#[derive(Clone, Copy)]
enum PipelineSource {
  QueuedPin,
  LatestPublished,
}

/// Checks a queued task before its next run and pins the selected pipeline, repository revision,
/// branch, and task limits in that run.
fn check(db: &ProjectDb, root: &Path, task_id: TaskId, source: PipelineSource) -> Result<Checked, Rejection> {
  let store = |error: StoreError| failed("cannot read the project database", &error);
  let task = db.task(task_id).map_err(store)?.ok_or(Rejection::TaskNotFound)?;
  check_readiness(db, task_id)?;
  // Unreadable runs count too: each run of the task has its own branch.
  let earlier = db
    .runs_of_task(task_id)
    .map_err(|error| failed("cannot read runs", &error))?;
  if task.status() != TaskStatus::Queued {
    return Err(Rejection::TaskNotQueued);
  }
  let pipeline_id = task
    .details()
    .pipeline
    .ok_or_else(|| preflight_failed(PreflightProblem::PipelineNotPublished))?;
  let pipeline = match source {
    PipelineSource::QueuedPin => {
      let number = task
        .pipeline_version()
        .ok_or_else(|| preflight_failed(PreflightProblem::PipelineVersionNotPinned))?;
      db.version::<Pipeline>(pipeline_id, number).map_err(store)?
    }
    PipelineSource::LatestPublished => db.latest_version::<Pipeline>(pipeline_id).map_err(store)?,
  };
  let published = Published::read(db).map_err(store)?;
  let connections = db.connections().map_err(store)?;
  let problems = preflight(PreflightInput {
    pipeline: pipeline.as_ref().map(Version::body),
    profiles: &published.profiles,
    tools: &published.tools,
    connections: &connections,
  });
  if !problems.is_empty() {
    return Err(Rejection::Preflight { problems });
  }
  let pipeline = pipeline.expect("the preflight rejects a pipeline without a published version");
  let (base, base_branch) = run_base(&db.base_branch_setting().map_err(store)?, root)?;
  let (project_sources, task_sources) = (
    db.project_sources().map_err(store)?,
    db.task_sources(task_id).map_err(store)?,
  );
  let snapshot = RunSnapshot::build(pipeline_id, &pipeline, published.versions())
    .map_err(|error| failed("cannot pin the run's configuration", &error))?
    .with_sources(&project_sources, &task_sources);
  let limits = task.details().limits;
  let new = NewRun {
    task: task_id,
    snapshot,
    branch: BranchName::for_next_run(task.key(), earlier.len()),
    base,
    base_branch,
    budget: limits.budget(),
    max_duration: limits.max_duration(),
  };
  Ok(Checked { task, new })
}

/// Records the run `checked` starts under `key`, with `revision` only while the task is still at it, then starts
/// it only if it is still pending.
pub(super) fn record(
  db: &mut ProjectDb,
  root: &Path,
  key: IdempotencyKey,
  Checked { task, new }: Checked,
  revision: Option<TaskRevision>,
) -> Result<Prepared, Rejection> {
  let run = db
    .create_run_of_queued_task(key, new, Timestamp::now(), revision)
    .map_err(|error| match error {
      RunStoreError::TaskNotQueued(_) => Rejection::TaskNotQueued,
      RunStoreError::TaskChangedMeanwhile(_) => Rejection::TaskChangedMeanwhile,
      other => read_failed("cannot record the run", other),
    })?;
  let task = db
    .task_for_run(task.id(), run.id())
    .map_err(|error| failed("cannot read the run's task snapshot", &error))?
    .ok_or(Rejection::TaskNotFound)?;
  finish_recorded(db, root, run, task)
}

/// A retry of a recorded request reads that run directly, without rechecking configuration that
/// may have changed since the first request. A pending record still needs its worktree and start.
pub(super) fn resume_recorded(
  db: &mut ProjectDb,
  root: &Path,
  task_id: TaskId,
  run: Run,
) -> Result<Prepared, Rejection> {
  if run.task() != task_id {
    return Err(Rejection::Failed {
      detail: "the start request key belongs to another task".into(),
    });
  }
  let task = db
    .task_for_run(task_id, run.id())
    .map_err(|error| failed("cannot read the run's task", &error))?
    .ok_or(Rejection::TaskNotFound)?;
  finish_recorded(db, root, run, task)
}

fn finish_recorded(db: &mut ProjectDb, root: &Path, mut run: Run, task: Task) -> Result<Prepared, Rejection> {
  let worktrees = Worktrees::new(root).map_err(|error| failed("cannot find the worktrees", &error))?;
  let own = worktrees.path(run.id());
  // A project that has not turned warm workspaces on gives each run a worktree of its own, as before them.
  let borrows = db
    .workspace_settings()
    .map_err(|error| failed("cannot read the workspace settings", &error))?
    .enabled;
  let folder = if own.is_dir() || !borrows {
    RunFolder::Own(own)
  } else {
    RunFolder::Borrowed
  };
  if let RunStatus::Failed(RunFailure::WorktreeNotCreated(reason)) = run.status() {
    return Err(Rejection::RunNotStarted {
      run: run.id(),
      why: reason.clone(),
    });
  }
  if run.status() != &RunStatus::Pending {
    return Ok(Prepared {
      run,
      task,
      folder,
      execute: false,
      try_at_plan_limit: None,
    });
  }
  check_readiness(db, run.task())?;
  match run_branch(&worktrees, &run, &folder) {
    Ok(branch) => record_branch(db, &mut run, branch)?,
    Err(reason) => return Err(fail_to_start(db, run, reason)),
  }
  db.record_run_origin(run.id(), origin_push_url(root).as_deref())
    .map_err(|error| failed("cannot record the run's origin", &error))?;
  if folder == RunFolder::Borrowed {
    db.set_run_borrows(run.id(), true)
      .map_err(|error| failed("cannot record that the run borrows workspaces", &error))?;
  }
  let events = run
    .start(Timestamp::now())
    .map_err(|error| failed("cannot start the run", &error))?;
  db.append_events(&run, &events)
    .map_err(|error| failed("cannot record the run's start", &error))?;
  Ok(Prepared {
    run,
    task,
    folder,
    execute: true,
    try_at_plan_limit: None,
  })
}

/// The branch of the pending run: the one its own worktree is on, which is created now unless an earlier request
/// did; or, for a run that borrows workspaces, a branch made now without a worktree. Why there is none, when it cannot
/// be. Only the runner's log has git's full answer.
fn run_branch(worktrees: &Worktrees, run: &Run, folder: &RunFolder) -> Result<BranchName, WorktreeFailure> {
  let branch = match folder {
    RunFolder::Own(path) if path.is_dir() => worktrees.branch_of(run.id()),
    RunFolder::Own(_) => worktrees
      .create(run.id(), run.branch().as_str(), run.base().as_str())
      .map(|created| created.branch().to_owned()),
    RunFolder::Borrowed => worktrees.create_branch(run.branch().as_str(), run.base().as_str()),
  }
  .map_err(|error| {
    tracing::error!(run = %run.id(), %error, "cannot create the run's branch");
    error.failure()
  })?;
  BranchName::new(&branch).map_err(|error| {
    tracing::error!(run = %run.id(), %branch, %error, "the run's worktree is on a branch a run cannot have");
    WorktreeFailure::InvalidBranch
  })
}

/// Fails the pending run whose worktree could not be created for `reason`: it would otherwise wait
/// for a start that cannot come. The answer says why; the run goes on only as a new one.
fn fail_to_start(db: &mut ProjectDb, mut run: Run, reason: WorktreeFailure) -> Rejection {
  tracing::error!(run = %run.id(), ?reason, "the run failed before it started");
  let recorded = run
    .fail_to_start(reason.clone(), Timestamp::now())
    .map_err(|error| failed("cannot fail the run that could not start", &error))
    .and_then(|events| {
      db.append_events(&run, &events)
        .map_err(|error| failed("cannot record that the run could not start", &error))
    });
  match recorded {
    Ok(()) => Rejection::RunNotStarted {
      run: run.id(),
      why: reason,
    },
    Err(rejection) => rejection,
  }
}

/// Records `branch`, where the run's worktree is, as the run's branch: a free name the worktree took
/// because a branch of the run's own existed, which merging, pull requests and the screens read.
fn record_branch(db: &mut ProjectDb, run: &mut Run, branch: BranchName) -> Result<(), Rejection> {
  let events = run
    .choose_branch(branch, Timestamp::now())
    .map_err(|error| failed("cannot move the run to its branch", &error))?;
  if events.is_empty() {
    return Ok(());
  }
  tracing::info!(run = %run.id(), branch = %run.branch(), "a branch of the run's name existed; the run works on a free one");
  db.append_events(run, &events)
    .map_err(|error| failed("cannot record the run's branch", &error))
}

fn check_readiness(db: &ProjectDb, task: TaskId) -> Result<(), Rejection> {
  match db.check_task_readiness(task) {
    Ok(()) => Ok(()),
    Err(StoreError::Organization(orchester_core::task::organization::OrganizationError::Blocked(_))) => {
      Err(Rejection::RunAgainBlocked {
        block: orchester_core::run::RunAgainBlock::TaskBlocked,
      })
    }
    Err(error) => Err(failed("cannot read task readiness", &error)),
  }
}

impl Prepared {
  /// A recorded run that continues where it works: resumed or with raised limits. A run whose worktree a person
  /// removed never continues, even should a folder of its name be back.
  pub(super) fn continuing(db: &ProjectDb, root: &Path, run: Run) -> Result<Self, Rejection> {
    check_readiness(db, run.task())?;
    if db
      .worktree_removal(run.id())
      .map_err(|error| failed("cannot read whether the run's worktree was removed", &error))?
      .is_some()
    {
      tracing::warn!(run = %run.id(), "a person removed the run's worktree; its agent cannot continue");
      return Err(Rejection::WorktreeRemoved);
    }
    let task = db
      .task_for_run(run.task(), run.id())
      .map_err(|error| failed("cannot read the run's task", &error))?
      .ok_or(Rejection::TaskNotFound)?;
    let folder = existing_folder(db, root, &run)?;
    Ok(Self {
      run,
      task,
      folder,
      execute: true,
      try_at_plan_limit: None,
    })
  }

  /// This continuation starting `execution` on its account once, though its plan limit was last reported reached.
  pub(super) const fn trying_at_plan_limit(mut self, execution: StepExecutionId) -> Self {
    self.try_at_plan_limit = Some(execution);
    self
  }
}

/// Where `run`, which continues, works: its own worktree while it is there, for a run made before workspaces; a
/// workspace it borrows, on its branch, for a run made since. Refused when a run of the first kind lost its worktree,
/// and when a borrowing run's branch, which holds all its work, is gone.
pub(super) fn existing_folder(db: &ProjectDb, root: &Path, run: &Run) -> Result<RunFolder, Rejection> {
  let own = Worktrees::new(root)
    .map_err(|error| failed("cannot find the worktrees", &error))?
    .path(run.id());
  if own.is_dir() {
    return Ok(RunFolder::Own(own));
  }
  let borrows = db
    .run_borrows(run.id())
    .map_err(|error| failed("cannot read whether the run borrows workspaces", &error))?;
  let branch = format!("refs/heads/{}", run.branch());
  let kept = Git::new(root)
    .succeeds(&["show-ref", "--verify", "--quiet", &branch])
    .map_err(|error| failed("cannot read the run's branch", &error))?;
  if !borrows || !kept {
    tracing::warn!(run = %run.id(), "the run's worktree or branch is gone; its agent cannot continue");
    return Err(Rejection::WorktreeRemoved);
  }
  Ok(RunFolder::Borrowed)
}

fn preflight_failed(problem: PreflightProblem) -> Rejection {
  Rejection::Preflight {
    problems: vec![problem],
  }
}

/// Where a push to `origin` goes as the run starts, which a push auto mode reviews must still go to;
/// `None` when the repository has no `origin` or git cannot tell, and then every push asks the person.
fn origin_push_url(root: &Path) -> Option<String> {
  match Git::new(root).attempt(&["remote", "get-url", "--push", "origin"]) {
    Ok(Ok(url)) => Some(url.trim().to_owned()).filter(|url| !url.is_empty()),
    Ok(Err(_)) => None,
    Err(error) => {
      tracing::warn!(%error, "cannot read the push URL of origin");
      None
    }
  }
}

/// Where a new run starts and the branch it delivers into, as the project's base branch `setting` says
/// (`orchester_core::integration::base`): the checkout's `HEAD` and the branch it has checked out (none on a detached
/// HEAD), or the tip of the branch set, wherever the checkout is. A branch set that is gone refuses the run.
fn run_base(setting: &BaseBranchSetting, root: &Path) -> Result<(Revision, Option<BranchName>), Rejection> {
  let runner = ProcessRunner::new(GIT_TIMEOUT);
  match setting {
    BaseBranchSetting::CheckedOut => {
      let base = base_revision(root)?;
      let branch = checked_out_branch(&runner, Path::new("git"), root)
        .map_err(|error| failed("cannot read the checked-out branch", &error))?;
      Ok((base, branch))
    }
    BaseBranchSetting::Branch(branch) => match set_branch_tip(&runner, Path::new("git"), (root, branch)) {
      Ok(tip) => Ok((tip, Some(branch.clone()))),
      Err(RunBaseError::Missing(branch)) => Err(preflight_failed(PreflightProblem::BaseBranchMissing { branch })),
      Err(RunBaseError::Git(error)) => Err(failed("cannot read the base branch", &error)),
    },
  }
}

/// `HEAD` of the user's checkout: the run starts from what the user has checked out.
fn base_revision(root: &Path) -> Result<Revision, Rejection> {
  let git = Git::new(root);
  match git.stdout(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]) {
    Ok(head) => Revision::new(&head).map_err(|error| failed("git reported an unexpected revision", &error)),
    // No commit to start from: find out whether there is a repository at all.
    Err(_) => match git.succeeds(&["rev-parse", "--git-dir"]) {
      Ok(true) => Err(preflight_failed(PreflightProblem::NoCommits)),
      Ok(false) => Err(preflight_failed(PreflightProblem::NotAGitRepository)),
      Err(error) => Err(failed("cannot run git", &error)),
    },
  }
}

/// The latest published version of every profile, tool, instruction and skill, which a snapshot
/// pins.
struct Published {
  profiles: BTreeMap<ResourceId, Version<AgentProfile>>,
  tools: BTreeMap<ResourceId, Version<Tool>>,
  instructions: BTreeMap<ResourceId, Version<Instruction>>,
  skills: BTreeMap<ResourceId, Version<Skill>>,
}

impl Published {
  fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    Ok(Self {
      profiles: latest(db, ResourceKind::AgentProfile)?,
      tools: latest(db, ResourceKind::Tool)?,
      instructions: latest(db, ResourceKind::Instruction)?,
      skills: latest(db, ResourceKind::Skill)?,
    })
  }

  fn versions(&self) -> PublishedVersions<'_> {
    PublishedVersions {
      profiles: &self.profiles,
      tools: &self.tools,
      instructions: &self.instructions,
      skills: &self.skills,
    }
  }
}

fn latest<B: ResourceBody>(db: &ProjectDb, kind: ResourceKind) -> Result<BTreeMap<ResourceId, Version<B>>, StoreError> {
  let mut versions = BTreeMap::new();
  for resource in db.resources(kind)? {
    if let Some(version) = db.latest_version::<B>(resource.id())? {
      versions.insert(resource.id(), version);
    }
  }
  Ok(versions)
}
