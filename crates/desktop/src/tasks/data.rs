//! Reading and saving tasks through the project database. Every function blocks on SQLite;
//! the UI calls them off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{AgentProfile, Pipeline, ResourceKind, VersionNumber},
  run::{Run, RunEvent, StepExecutor},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  detail::run_store_error,
  list::{AwaitedPlan, LatestRun, Phases},
};

/// A pipeline a task can use: one with a published version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  /// The latest published version, which new runs use.
  pub version: VersionNumber,
  pub steps: Vec<Arc<str>>,
}

/// The project's tasks and the pipelines they can use, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTasks {
  pub tasks: Arc<Vec<Task>>,
  pub pipelines: Arc<Vec<PipelineChoice>>,
  /// The phase of each task that has a run, from its latest run.
  pub phases: Arc<Phases>,
  /// Runs started in the project, of every task.
  pub runs: usize,
  /// Runs recorded as running that lost their agent because no runner is connected; 0 while
  /// one is ([`Self::without_runner`]).
  pub lost_to_runner: usize,
}

impl lurq::app::component::DevtoolsInspectable for ProjectTasks {}

impl ProjectTasks {
  pub fn load(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let tasks = db.tasks()?;
    let mut phases = Phases::new();
    let mut count = 0;
    for task in &tasks {
      let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
      count += runs.len();
      if let Some(latest) = runs.last() {
        let updated_at = db
          .events(latest.id())
          .map_err(run_store_error)?
          .last()
          .map_or_else(|| latest.created_at(), RunEvent::at);
        let plan = awaited_plan(&db, latest)?;
        let merge = db.run_merge(latest.id())?;
        phases.insert(
          task.id(),
          LatestRun::of(task, (latest, merge.as_ref()), updated_at, plan),
        );
      }
    }
    Ok(Self {
      tasks: Arc::new(tasks),
      pipelines: Arc::new(published_pipelines(&db)?),
      phases: Arc::new(phases),
      runs: count,
      lost_to_runner: 0,
    })
  }

  /// The tasks as they stand while no runner is connected: runs recorded as running are
  /// interrupted ([`orchester_core::run::TaskPhase::without_runner`]).
  #[must_use]
  pub fn without_runner(&self) -> Self {
    let phases: Phases = self
      .phases
      .iter()
      .map(|(task, latest)| (*task, latest.without_runner()))
      .collect();
    let lost_to_runner = phases
      .iter()
      .filter(|(task, latest)| self.phases.get(task).is_some_and(|before| before.phase != latest.phase))
      .count();
    Self {
      phases: Arc::new(phases),
      lost_to_runner,
      ..self.clone()
    }
  }
}

/// The plan `run` waits on a decision for, with the agent role of the step that proposed it; only
/// such a run needs its plans and a profile read here.
fn awaited_plan(db: &ProjectDb, run: &Run) -> Result<Option<AwaitedPlan>, StoreError> {
  if run.awaiting_plan().is_empty() {
    return Ok(None);
  }
  let plans = db.plans(run.id()).map_err(run_store_error)?;
  let mut roles = BTreeMap::new();
  for step in run.snapshot().steps() {
    if let StepExecutor::Agent { profile, .. } = step.executor()
      && let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)?
    {
      roles.insert(step.key().clone(), Arc::<str>::from(version.body().role().as_str()));
    }
  }
  Ok(AwaitedPlan::of(run, &plans, |step| roles.get(step).cloned()))
}

/// Pipelines with a published version, ordered by slug. Drafts that were never published
/// cannot be run, so they are not offered.
pub fn published_pipelines<S: ResourceStore>(store: &S) -> Result<Vec<PipelineChoice>, S::Error> {
  let mut choices = Vec::new();
  for resource in store.resources(ResourceKind::Pipeline)? {
    let Some(version) = store.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    choices.push(PipelineChoice {
      id: resource.id(),
      name: resource.name().as_str().into(),
      version: version.number(),
      steps: version
        .body()
        .steps()
        .iter()
        .map(|step| step.name.as_str().into())
        .collect(),
    });
  }
  Ok(choices)
}

/// Saves `details` as one draft task under `batch`, the key the form generated for this
/// save. Retrying with the same key never creates a second task: if an earlier attempt was
/// stored after all, that task is returned, with the details of this attempt if they were
/// edited since and the task is still a draft.
pub fn save_draft<S: TaskStore>(
  store: &mut S,
  batch: IdempotencyKey,
  details: TaskDetails,
  now: Timestamp,
) -> Result<Task, S::Error> {
  let task = store
    .create_tasks(batch, vec![details.clone()], now)?
    .into_iter()
    .next()
    .expect("create_tasks returns one task for each of the one details given");
  if task.details() == &details || task.status() != TaskStatus::Draft {
    return Ok(task);
  }
  let edited = Task::from_stored(task.id(), task.key().clone(), details, task.status(), task.created_at());
  store.save_task(&edited)?;
  Ok(edited)
}

/// [`save_draft`] in the project database of `repo_root`.
pub fn save_draft_in(repo_root: &Path, batch: IdempotencyKey, details: TaskDetails) -> Result<Task, StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  save_draft(&mut db, batch, details, Timestamp::now())
}
