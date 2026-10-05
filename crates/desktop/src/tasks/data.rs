//! Reading and saving tasks through the project database. Every function blocks on SQLite;
//! the UI calls them off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  conversation::{Message, latest_question},
  id::{IdempotencyKey, ResourceId, RunId, TaskId},
  resource::{AgentProfile, Pipeline, ResourceKind, Step, StepKey, VersionNumber},
  run::{Run, RunEvent, RunNumber, StepExecutor, TaskPhase},
  sharing::TaskRevision,
  store::{ConversationStore, MergeStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskStatus, TaskUpdate},
  task_group::GroupHold,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  approvals::{self, ShownApproval},
  detail::run_store_error,
  list::{Awaited, AwaitedPlan, AwaitedQuestion, Latest, LatestRun, Phases, phase_of},
  overview::OverviewTab,
  run_approvals,
  unreadable::Unreadable,
};
use crate::{
  messages::Message as UiMessage,
  pipelines::{data::Assignables, describe},
};

/// The tasks kept *Not shared* when the project joined, with the tag names each kept.
type Kept = (Vec<Task>, std::collections::HashMap<TaskId, Vec<String>>);

fn kept(db: &ProjectDb) -> Result<Kept, StoreError> {
  let tasks = db.not_shared_tasks()?;
  let mut tags = std::collections::HashMap::new();
  for task in &tasks {
    if let Some(names) = db.shared_task_info(task.id())?.and_then(|info| info.kept_tags) {
      tags.insert(task.id(), names);
    }
  }
  Ok((tasks, tags))
}

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
  pub updated: Arc<std::collections::HashMap<TaskId, Timestamp>>,
  pub tags: Arc<Vec<orchester_core::task::organization::Tag>>,
  pub blocking: Arc<std::collections::HashMap<TaskId, Vec<TaskId>>>,
  pub tasks: Arc<Vec<Task>>,
  /// The tasks kept *Not shared* when the project joined a server project, out of `tasks`.
  pub not_shared: Arc<Vec<Task>>,
  /// The tag names each *Not shared* task kept.
  pub kept_tags: Arc<std::collections::HashMap<TaskId, Vec<String>>>,
  /// The key each task had before it was uploaded (*was KON-12*), which the search finds.
  pub former_keys: Arc<std::collections::HashMap<TaskId, String>>,
  pub pipelines: Arc<Vec<PipelineChoice>>,
  /// Shared tasks whose pipeline this computer lacks, with the pipeline's slug.
  pub missing_pipelines: Arc<std::collections::HashMap<TaskId, String>>,
  /// Why each task of a reviewed group that holds its work waits.
  pub group_holds: Arc<BTreeMap<TaskId, GroupHold>>,
  /// The tasks of reviewed groups, which a group starts in its turn unless they are Canceled.
  pub grouped: Arc<std::collections::BTreeSet<TaskId>>,
  /// Tool requests awaiting a decision anywhere in this project, oldest first.
  pub pending_tool_approvals: Arc<Vec<ShownApproval>>,
  /// The phase of each task that has a run, from its latest run, or that it cannot be read.
  pub phases: Arc<Phases>,
  /// How many runs of each task were started and can be read.
  pub runs: Arc<BTreeMap<TaskId, usize>>,
  /// Runs recorded as running that lost their agent because no runner is connected; 0 while
  /// one is ([`Self::without_runner`]).
  pub lost_to_runner: usize,
  /// The active runs that auto-approve their agents' requests, oldest task first.
  pub auto_approving: Arc<Vec<AutoApprovingRun>>,
}

/// An active run that auto-approves, as the status bar names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoApprovingRun {
  pub run: RunId,
  pub number: RunNumber,
  pub task: TaskId,
  pub key: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for ProjectTasks {}

impl ProjectTasks {
  /// `row` with its tag names and what blocks it, the blocking tasks' keys joined by `separator`
  /// (`common.listSeparator`).
  pub fn decorate_row(&self, mut row: super::list::TaskRow, separator: &str) -> super::list::TaskRow {
    if let Some(slug) = self.missing_pipelines.get(&row.id) {
      row.pipeline = super::list::PipelineLabel::Missing {
        slug: Arc::from(slug.as_str()),
      };
    }
    row.tags = self
      .tags
      .iter()
      .filter(|tag| row.organization.tags().contains(&tag.id()))
      .map(|tag| Arc::from(tag.name().as_str()))
      .collect();
    row.former_key = self.former_keys.get(&row.id).map(|key| Arc::from(key.as_str()));
    if let Some(kept) = self.kept_tags.get(&row.id) {
      row.tags = kept.iter().map(|name| Arc::from(name.as_str())).collect();
      row.not_shared = true;
      row.detail = UiMessage::new("tasks.notShared.readiness");
      return row;
    }
    if let Some(blocking) = self.blocking.get(&row.id).filter(|links| !links.is_empty()) {
      let keys = blocking
        .iter()
        .filter_map(|id| self.tasks.iter().find(|task| task.id() == *id))
        .map(|task| task.key().to_string())
        .collect::<Vec<_>>()
        .join(separator);
      row.detail = UiMessage::new("tasks.blocked.rowUnfinished").with("tasks", keys);
    } else if let Some(reason) = row.organization.blocked_reason() {
      row.detail = UiMessage::new("tasks.blocked.informational").with("reason", reason.to_owned());
    }
    row
  }

  /// Runs started of the tasks in `rows` that can be read.
  #[must_use]
  pub fn runs_of(&self, rows: &[super::list::TaskRow]) -> usize {
    rows.iter().filter_map(|row| self.runs.get(&row.id)).sum()
  }

  /// How many of `tasks` have a run that has not ended, which bulk Canceled stops. A latest run
  /// that cannot be read counts: the store, which decides, may find it active.
  pub fn runs_not_ended(&self, tasks: &std::collections::BTreeSet<TaskId>) -> usize {
    self
      .phases
      .iter()
      .filter(|(id, latest)| {
        tasks.contains(id) && latest.phase().is_none_or(orchester_core::run::TaskPhase::run_not_ended)
      })
      .count()
  }

  /// The *Not shared* tasks in place of the list's, for the list's *Not shared* filter (10.23).
  #[must_use]
  pub fn not_shared_view(&self) -> Self {
    Self {
      tasks: self.not_shared.clone(),
      ..self.clone()
    }
  }

  pub fn load(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let tasks = db.tasks()?;
    let (not_shared, kept_tags) = kept(&db)?;
    let pending = approvals::pending(&db, repo_root)?;
    let mut group_holds = BTreeMap::new();
    let mut grouped = std::collections::BTreeSet::new();
    for group in db.task_groups()? {
      grouped.extend(group.tasks().iter().copied());
      if let Some(hold) = db.task_group_hold(group.key())? {
        for task in group.tasks() {
          group_holds.insert(*task, hold.clone());
        }
      }
    }
    let mut phases = Phases::new();
    let mut counts = BTreeMap::new();
    let mut auto_approving = Vec::new();
    for task in tasks.iter().chain(&not_shared) {
      let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
      counts.insert(task.id(), runs.iter().filter(|run| run.is_ok()).count());
      let latest = match runs.last() {
        Some(Ok(latest)) => {
          if db.task_group_for_run(latest).map_err(run_store_error)?.is_none() {
            group_holds.remove(&task.id());
          }
          let mut shown = latest_run(&db, task, latest)?;
          // A run whose agent waits for a tool decision waits for a person, as one at a plan does.
          if shown.phase == TaskPhase::Running && pending.iter().any(|shown| shown.approval.run == latest.id()) {
            shown.phase = TaskPhase::Waiting;
          }
          if shown.phase.is_active() && run_approvals::auto_approving(&db, latest)? {
            auto_approving.push(AutoApprovingRun {
              run: latest.id(),
              number: latest.number(),
              task: task.id(),
              key: task.key().to_string().into(),
            });
          }
          Latest::Run(shown)
        }
        Some(Err(unreadable)) => Latest::Unreadable(Unreadable::of(unreadable)),
        None => continue,
      };
      phases.insert(task.id(), latest);
    }
    Ok(Self {
      updated: Arc::new(
        tasks
          .iter()
          .map(|task| {
            db.task_organization_updated_at(task.id())
              .map(|updated| (task.id(), updated.unwrap_or(task.created_at())))
          })
          .collect::<Result<_, _>>()?,
      ),
      tags: Arc::new(db.project_tags()?),
      blocking: Arc::new(
        tasks
          .iter()
          .map(|task| db.unfinished_blocking_tasks(task.id()).map(|links| (task.id(), links)))
          .collect::<Result<_, _>>()?,
      ),
      tasks: Arc::new(tasks),
      not_shared: Arc::new(not_shared),
      kept_tags: Arc::new(kept_tags),
      former_keys: Arc::new(db.former_keys()?),
      group_holds: Arc::new(group_holds),
      grouped: Arc::new(grouped),
      pipelines: Arc::new(published_pipelines(&db)?),
      missing_pipelines: Arc::new(db.missing_pipelines()?),
      pending_tool_approvals: Arc::new(pending),
      phases: Arc::new(phases),
      runs: Arc::new(counts),
      lost_to_runner: 0,
      auto_approving: Arc::new(auto_approving),
    })
  }

  /// What *Needs you* counts: each request waiting for a decision, and each other task waiting for a
  /// person (a plan, a question). Whether the dialog is open changes nothing.
  pub fn needs_you(&self) -> usize {
    let asking: Vec<TaskId> = self
      .pending_tool_approvals
      .iter()
      .filter_map(|shown| shown.task_id)
      .collect();
    let other_waiting = self
      .tasks
      .iter()
      .filter(|task| OverviewTab::NeedsYou.includes(phase_of(task, &self.phases)) && !asking.contains(&task.id()))
      .count();
    other_waiting + self.pending_tool_approvals.len()
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
) -> Result<TaskUpdate, S::Error> {
  let task = store
    .create_tasks(batch, vec![details.clone()], now)?
    .into_iter()
    .next()
    .expect("create_tasks returns one task for each of the one details given");
  if task.details() == &details || task.status() != TaskStatus::Draft {
    return Ok(TaskUpdate::Saved(task));
  }
  let edited = Task::from_stored(
    task.id(),
    task.key().clone(),
    details,
    task.status(),
    task.pipeline_version(),
    task.created_at(),
  )
  .with_organization(task.organization().clone())
  .with_revision(task.revision());
  store.save_task(&edited)
}

/// Where the task form saves: a new draft under `New`, or an existing task in place, as it was
/// read at the revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTarget {
  New(IdempotencyKey),
  Task(TaskId, TaskRevision),
}

/// Saves `details` as a new draft or over an existing task in place.
pub fn save_to<S: TaskStore>(
  store: &mut S,
  target: SaveTarget,
  details: TaskDetails,
  now: Timestamp,
) -> Result<TaskUpdate, S::Error> {
  match target {
    SaveTarget::New(batch) => save_draft(store, batch, details, now),
    SaveTarget::Task(task, read) => store.update_task(task, read, details),
  }
}

/// A Save as draft request cannot succeed when its idempotency key now names a queued task.
#[derive(Debug, Error)]
pub enum TaskDraftSaveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The shared project's server did not take the save.
  #[error(transparent)]
  Write(Box<super::writer::WriteError>),
  #[error("the task is no longer a draft: {0:?}")]
  NotDraft(TaskStatus),
  #[error("the persisted task differs from the requested details")]
  DetailsMismatch,
  /// The task was changed since the form read it; nothing was saved.
  #[error("the task was changed since it was read")]
  Changed(Box<Task>),
}

impl From<super::writer::WriteError> for TaskDraftSaveError {
  fn from(error: super::writer::WriteError) -> Self {
    match error {
      super::writer::WriteError::Store(error) => Self::Store(error),
      error => Self::Write(Box::new(error)),
    }
  }
}

/// [`save_to`] in the project database of `repo_root`, confirmed as the requested details: the
/// saved task.
pub fn save_draft_in(repo_root: &Path, target: SaveTarget, details: &TaskDetails) -> Result<Task, TaskDraftSaveError> {
  let mut db = ProjectDb::open(repo_root)?;
  let saved = match save_to(&mut db, target, details.clone(), Timestamp::now())? {
    TaskUpdate::Saved(task) => task,
    TaskUpdate::Conflict { current } => return Err(TaskDraftSaveError::Changed(Box::new(current))),
  };
  if matches!(target, SaveTarget::New(_)) && saved.status() != TaskStatus::Draft {
    return Err(TaskDraftSaveError::NotDraft(saved.status()));
  }
  if saved.details() != details {
    return Err(TaskDraftSaveError::DetailsMismatch);
  }
  Ok(saved)
}
