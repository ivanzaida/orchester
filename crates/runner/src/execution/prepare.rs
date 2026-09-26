//! Turning a queued task into a started run: preflight, the run's base revision and branch, its
//! record in the database, its worktree and its start. Idempotent by the request's key.

use std::{
  collections::BTreeMap,
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, TaskId},
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Tool, Version},
  revision::Revision,
  run::{
    BranchName, NewRun, PreflightInput, PreflightProblem, PublishedVersions, Run, RunSnapshot, RunStatus, preflight,
  },
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskStatus},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::failed;
use crate::{git::Git, worktree::Worktrees};

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
  let store = |error: StoreError| failed("cannot read the project database", &error);
  let task = db.task(task_id).map_err(store)?.ok_or(Rejection::TaskNotFound)?;
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
    connections: &connections,
  });
  if !problems.is_empty() {
    return Err(Rejection::Preflight { problems });
  }
  let pipeline = pipeline.expect("the preflight rejects a pipeline without a published version");
  let base = base_revision(root)?;
  let snapshot = RunSnapshot::build(pipeline_id, &pipeline, published.versions())
    .map_err(|error| failed("cannot pin the run's configuration", &error))?;
  let ordinal = u32::try_from(earlier.len() + 1)
    .ok()
    .and_then(NonZeroU32::new)
    .expect("fewer than 2^32 runs of one task");
  let new = NewRun {
    task: task_id,
    snapshot,
    branch: BranchName::for_run(task.key(), ordinal),
    base,
    budget: task.details().limits.expect("a queued task has limits").budget(),
  };
  let mut run = db
    .create_run(key, new, Timestamp::now())
    .map_err(|error| failed("cannot record the run", &error))?;
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

fn preflight_failed(problem: PreflightProblem) -> Rejection {
  Rejection::Preflight {
    problems: vec![problem],
  }
}

/// `HEAD` of the user's checkout: the run starts from what the user has checked out.
fn base_revision(root: &Path) -> Result<Revision, Rejection> {
  let git = Git::new(root);
  match git.stdout(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]) {
    Ok(head) => Revision::new(&head).map_err(|error| failed("git reported an unexpected revision", &error)),
    // No commit to start from: find out whether there is a repository at all.
    Err(_) => match git.succeeds(["rev-parse", "--git-dir"]) {
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
