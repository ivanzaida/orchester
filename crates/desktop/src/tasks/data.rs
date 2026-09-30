//! Reading and saving tasks through the project database. Every function blocks on SQLite;
//! the UI calls them off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  conversation::{Message, latest_question},
  id::{IdempotencyKey, ResourceId, TaskId},
  resource::{AgentProfile, Pipeline, ResourceKind, Step, StepKey, VersionNumber},
  run::{PendingToolApproval, Run, RunEvent, StepExecutor},
  store::{ConversationStore, MergeStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  detail::run_store_error,
  list::{Awaited, AwaitedPlan, AwaitedQuestion, Latest, LatestRun, Phases},
  unreadable::Unreadable,
};
use crate::{
  messages::Message as UiMessage,
  pipelines::{data::Assignables, describe},
};

/// A step in a published pipeline a task can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineStepChoice {
  pub name: Arc<str>,
  pub executor: UiMessage,
  pub approval_gate: bool,
}

/// A pipeline a task can use: one with a published version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  /// The published version this choice represents.
  pub version: VersionNumber,
  pub steps: Vec<PipelineStepChoice>,
}

/// The project's tasks and the pipelines they can use, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTasks {
  pub tasks: Arc<Vec<Task>>,
  pub pipelines: Arc<Vec<PipelineChoice>>,
  pub group_holds: Arc<BTreeMap<TaskId, Vec<UiMessage>>>,
  /// Tool requests awaiting a decision anywhere in this project, oldest first.
  pub pending_tool_approvals: Arc<Vec<PendingToolApproval>>,
  /// The phase of each task that has a run, from its latest run, or that it cannot be read.
  pub phases: Arc<Phases>,
  /// Runs started in the project, of every task, that can be read.
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
    let mut group_holds = BTreeMap::new();
    for group in db.task_groups()? {
      if let Some(hold) = db.task_group_hold(group.key())? {
        let messages = super::group::hold_messages(&hold);
        for task in group.tasks() {
          group_holds.insert(*task, messages.clone());
        }
      }
    }
    let mut phases = Phases::new();
    let mut count = 0;
    for task in &tasks {
      let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
      count += runs.iter().filter(|run| run.is_ok()).count();
      let latest = match runs.last() {
        Some(Ok(latest)) => Latest::Run(latest_run(&db, task, latest)?),
        Some(Err(unreadable)) => Latest::Unreadable(Unreadable::of(unreadable)),
        None => continue,
      };
      phases.insert(task.id(), latest);
    }
    Ok(Self {
      tasks: Arc::new(tasks),
      group_holds: Arc::new(group_holds),
      pipelines: Arc::new(published_pipelines(&db)?),
      pending_tool_approvals: Arc::new(db.pending_tool_approvals_all()?),
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
      .filter(|(task, latest)| {
        self
          .phases
          .get(task)
          .is_some_and(|before| before.phase() != latest.phase())
      })
      .count();
    Self {
      phases: Arc::new(phases),
      lost_to_runner,
      ..self.clone()
    }
  }
}

/// `latest`, a run of `task`, with when it last changed and what it waits for.
pub(super) fn latest_run(db: &ProjectDb, task: &Task, latest: &Run) -> Result<LatestRun, StoreError> {
  let updated_at = db
    .events(latest.id())
    .map_err(run_store_error)?
    .last()
    .map_or_else(|| latest.created_at(), RunEvent::at);
  let awaited = awaited(db, latest)?;
  let merge = db.run_merge(latest.id())?;
  Ok(LatestRun::of(task, (latest, merge.as_ref()), updated_at, awaited))
}

/// The plan `run` waits on a decision for, or the question it waits for an answer to, with the agent
/// role of the step that proposed or asked it; only such a run needs its plans or its conversation
/// and its profiles read here.
fn awaited(db: &ProjectDb, run: &Run) -> Result<Awaited, StoreError> {
  let (plan_waits, question_waits) = (!run.awaiting_plan().is_empty(), !run.awaiting_answer().is_empty());
  if !plan_waits && !question_waits {
    return Ok(Awaited::default());
  }
  let mut roles = BTreeMap::new();
  for step in run.snapshot().steps() {
    if let StepExecutor::Agent { profile, .. } = step.executor()
      && let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)?
    {
      roles.insert(step.key().clone(), Arc::<str>::from(version.body().role().as_str()));
    }
  }
  let role = |step: &StepKey| roles.get(step).cloned();
  let plan = if plan_waits {
    AwaitedPlan::of(run, &db.plans(run.id()).map_err(run_store_error)?, role)
  } else {
    None
  };
  let question = if question_waits {
    awaited_question(run, &db.messages(run.task())?, role)
  } else {
    None
  };
  Ok(Awaited { plan, question })
}

/// The question the first execution waiting for an answer asked, among `messages`, and who asks.
fn awaited_question(
  run: &Run,
  messages: &[Message],
  role: impl Fn(&StepKey) -> Option<Arc<str>>,
) -> Option<AwaitedQuestion> {
  let execution = *run.awaiting_answer().first()?;
  let question = latest_question(messages, execution)?;
  let step = run.execution(execution)?.step();
  let agent = role(step).or_else(|| run.snapshot().body().step(step).map(|step| step.name.as_str().into()))?;
  Some(AwaitedQuestion {
    agent,
    text: question.body().as_str().into(),
  })
}

/// Pipelines with a published version, ordered by slug. Drafts that were never published
/// cannot be run, so they are not offered.
pub fn published_pipelines(store: &ProjectDb) -> Result<Vec<PipelineChoice>, StoreError> {
  let mut choices = Vec::new();
  let assignables = Assignables::load(store)?;
  for resource in store.resources(ResourceKind::Pipeline)? {
    let Some(version) = store.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    choices.push(pipeline_choice(&resource, &version, &assignables));
  }
  Ok(choices)
}

/// Reads the exact published pipeline pinned to a queued task.
pub fn pinned_pipeline(
  store: &ProjectDb,
  id: ResourceId,
  number: VersionNumber,
) -> Result<Option<PipelineChoice>, StoreError> {
  let Some(version) = store.version::<Pipeline>(id, number)? else {
    return Ok(None);
  };
  let resource = store.resource(id)?.ok_or(StoreError::ResourceNotFound(id))?;
  let assignables = Assignables::load(store)?;
  Ok(Some(pipeline_choice(&resource, &version, &assignables)))
}

fn pipeline_choice(
  resource: &orchester_core::resource::Resource,
  version: &orchester_core::resource::Version<Pipeline>,
  assignables: &Assignables,
) -> PipelineChoice {
  PipelineChoice {
    id: resource.id(),
    name: resource.name().as_str().into(),
    version: version.number(),
    steps: step_choices(version.body().steps(), assignables),
  }
}

fn step_choices(steps: &[Step], assignables: &Assignables) -> Vec<PipelineStepChoice> {
  steps
    .iter()
    .map(|step| PipelineStepChoice {
      name: step.name.as_str().into(),
      executor: describe::executor_line(step, assignables),
      approval_gate: step.approval_gate,
    })
    .collect()
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
  let edited = Task::from_stored(
    task.id(),
    task.key().clone(),
    details,
    task.status(),
    task.pipeline_version(),
    task.created_at(),
  );
  store.save_task(&edited)?;
  Ok(edited)
}

/// Where the task form saves: a new draft under `New`, or an existing task in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTarget {
  New(IdempotencyKey),
  Task(TaskId),
}

/// Saves `details` as a new draft or over an existing task in place.
pub fn save_to<S: TaskStore>(
  store: &mut S,
  target: SaveTarget,
  details: TaskDetails,
  now: Timestamp,
) -> Result<Task, S::Error> {
  match target {
    SaveTarget::New(batch) => save_draft(store, batch, details, now),
    SaveTarget::Task(task) => store.update_task(task, details),
  }
}

/// A Save as draft request cannot succeed when its idempotency key now names a queued task.
#[derive(Debug, Error)]
pub enum TaskDraftSaveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("the task is no longer a draft: {0:?}")]
  NotDraft(TaskStatus),
  #[error("the persisted task differs from the requested details")]
  DetailsMismatch,
}

/// [`save_to`] in the project database of `repo_root`, confirmed as the requested details.
pub fn save_draft_in(
  repo_root: &Path,
  target: SaveTarget,
  details: TaskDetails,
) -> Result<TaskDetails, TaskDraftSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let saved = save_to(&mut db, target, details.clone(), Timestamp::now())?;
  if matches!(target, SaveTarget::New(_)) && saved.status() != TaskStatus::Draft {
    return Err(TaskDraftSaveError::NotDraft(saved.status()));
  }
  if saved.details() != &details {
    return Err(TaskDraftSaveError::DetailsMismatch);
  }
  Ok(details)
}
