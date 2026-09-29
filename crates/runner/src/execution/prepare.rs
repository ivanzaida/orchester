//! Turning a queued task into a started run: preflight, the run's base revision and the branch
//! the user's checkout has there (which merging the run brings its work into), the run's branch,
//! its record in the database, its worktree and its start. Idempotent by the request's key.

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId, TaskId},
  integration::checked_out_branch,
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Tool, Version},
  revision::Revision,
  run::{
    BranchName, NewRun, PreflightInput, PreflightProblem, PublishedVersions, Run, RunSnapshot, RunStatus, preflight,
  },
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskStatus},
  time::Timestamp,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::{failed, read_failed};
use crate::{git::Git, worktree::Worktrees};

/// A git query that does not answer within this time counts as failed.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// A recorded run and whether this request is the one that executes it.
pub(super) struct Prepared {
  pub(super) run: Run,
  pub(super) task: Task,
  pub(super) worktree: PathBuf,
  /// `false` when an earlier request with the same key already started the run.
  pub(super) execute: bool,
}

pub(super) fn prepare(
  db: &mut ProjectDb,
  root: &Path,
  key: IdempotencyKey,
  task_id: TaskId,
) -> Result<Prepared, Rejection> {
  let checked = check(db, root, task_id)?;
  record(db, root, key, checked)
}

/// A queued task that passed the preflight, and the run it starts.
pub(super) struct Checked {
  task: Task,
  new: NewRun,
}

/// Checks that queued task `task_id` can start its next run and pins what that run uses: the
/// latest published version of its pipeline, the repository's `HEAD` and branch, the task's
/// limits and the branch of its next run.
pub(super) fn check(db: &ProjectDb, root: &Path, task_id: TaskId) -> Result<Checked, Rejection> {
  let store = |error: StoreError| failed("cannot read the project database", &error);
  let task = db.task(task_id).map_err(store)?.ok_or(Rejection::TaskNotFound)?;
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
  let pipeline = db.latest_version::<Pipeline>(pipeline_id).map_err(store)?;
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
  let base = base_revision(root)?;
  let base_branch = checked_out_branch(&ProcessRunner::new(GIT_TIMEOUT), Path::new("git"), root)
    .map_err(|error| failed("cannot read the checked-out branch", &error))?;
  let snapshot = RunSnapshot::build(pipeline_id, &pipeline, published.versions())
    .map_err(|error| failed("cannot pin the run's configuration", &error))?;
  let limits = task.details().limits.expect("a queued task has limits");
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

/// Records the run `checked` starts under `key`, creates its worktree and starts it; finds the run
/// an earlier request with `key` created instead, and starts it only if it had not started yet.
pub(super) fn record(
  db: &mut ProjectDb,
  root: &Path,
  key: IdempotencyKey,
  Checked { task, new }: Checked,
) -> Result<Prepared, Rejection> {
  let mut run = db
    .create_run(key, new, Timestamp::now())
    .map_err(|error| read_failed("cannot record the run", error))?;
  let worktrees = Worktrees::new(root).map_err(|error| failed("cannot find the worktrees", &error))?;
  let worktree = worktrees.path(run.id());
  if run.status() != &RunStatus::Pending {
    return Ok(Prepared {
      run,
      task,
      worktree,
      execute: false,
    });
  }
  if !worktree.is_dir() {
    worktrees
      .create(run.id(), run.branch().as_str(), run.base().as_str())
      .map_err(|error| failed("cannot create the run's worktree", &error))?;
  }
  let events = run
    .start(Timestamp::now())
    .map_err(|error| failed("cannot start the run", &error))?;
  db.append_events(&run, &events)
    .map_err(|error| failed("cannot record the run's start", &error))?;
  Ok(Prepared {
    run,
    task,
    worktree,
    execute: true,
  })
}

impl Prepared {
  /// A recorded run that continues in its existing worktree: resumed or with raised limits.
  pub(super) fn continuing(db: &ProjectDb, root: &Path, run: Run) -> Result<Self, Rejection> {
    let task = db
      .task(run.task())
      .map_err(|error| failed("cannot read the run's task", &error))?
      .ok_or(Rejection::TaskNotFound)?;
    let worktree = existing_worktree(root, run.id())?;
    Ok(Self {
      run,
      task,
      worktree,
      execute: true,
    })
  }
}

/// The worktree of `run`, which continues in it; refused when it was removed, as a person can once
/// the run ended, or its folder was deleted.
pub(super) fn existing_worktree(root: &Path, run: RunId) -> Result<PathBuf, Rejection> {
  let worktree = Worktrees::new(root)
    .map_err(|error| failed("cannot find the worktrees", &error))?
    .path(run);
  if !worktree.is_dir() {
    tracing::warn!(%run, worktree = %worktree.display(), "the run's worktree was removed; its agent cannot continue");
    return Err(Rejection::WorktreeRemoved);
  }
  Ok(worktree)
}

fn preflight_failed(problem: PreflightProblem) -> Rejection {
  Rejection::Preflight {
    problems: vec![problem],
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

/// The latest published version of every profile, tool and instruction, which a snapshot pins.
struct Published {
  profiles: BTreeMap<ResourceId, Version<AgentProfile>>,
  tools: BTreeMap<ResourceId, Version<Tool>>,
  instructions: BTreeMap<ResourceId, Version<Instruction>>,
}

impl Published {
  fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    Ok(Self {
      profiles: latest(db, ResourceKind::AgentProfile)?,
      tools: latest(db, ResourceKind::Tool)?,
      instructions: latest(db, ResourceKind::Instruction)?,
    })
  }

  fn versions(&self) -> PublishedVersions<'_> {
    PublishedVersions {
      profiles: &self.profiles,
      tools: &self.tools,
      instructions: &self.instructions,
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
