//! Reading and saving tasks through the project database. Every function blocks on SQLite;
//! the UI calls them off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  conversation::{Message, latest_question},
  id::{ResourceId, RunId, StepExecutionId, TaskId},
  limits::ProjectLimits,
  memory::{Memory, MemoryFilter, MemoryStatus, MemoryStore, ScopeFilter},
  question::AskedQuestions,
  resource::{AgentProfile, Pipeline, ResourceKind, Step, StepKey, VersionNumber},
  run::{Run, RunEvent, RunNumber, RunStatus, StepExecutor, TaskPhase, can_run_again_after},
  store::{ConversationStore, MergeStore, ResourceStore, RunStore, SettingsStore, TaskStore, WorktreeStore},
  task::Task,
  task_group::GroupHold,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

pub use super::draft_save::{SaveTarget, TaskDraftSaveError, save_draft, save_draft_in, save_to};
use super::{
  approvals::{self, ShownApproval},
  delivery::Unmerged,
  detail::run_store_error,
  list::{AwaitedPlan, AwaitedQuestion, AwaitedStep, CurrentStep, Latest, LatestRun, Phases, StepWait},
  needs_you::WaitingOnce,
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
  /// A tool runs the step, not an agent.
  pub tool: bool,
  /// The step is the starter pipeline's *Create pull request* (by its key): it opens a pull request.
  pub pull_request: bool,
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
  /// What a run of this version delivers, unless a task says otherwise (ORC-93).
  pub delivers: orchester_core::resource::Deliverable,
}

/// The project's tasks and the pipelines they can use, read together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTasks {
  pub updated: Arc<std::collections::HashMap<TaskId, Timestamp>>,
  pub tags: Arc<Vec<orchester_core::task::organization::Tag>>,
  /// The project's modules, oldest first; each task names its own (`Task::module`).
  pub modules: Arc<Vec<orchester_core::module::Module>>,
  pub blocking: Arc<std::collections::HashMap<TaskId, Vec<TaskId>>>,
  pub tasks: Arc<Vec<Task>>,
  /// The tasks kept *Not shared* when the project joined a server project, out of `tasks`.
  pub not_shared: Arc<Vec<Task>>,
  /// The tag names each *Not shared* task kept.
  pub kept_tags: Arc<std::collections::HashMap<TaskId, Vec<String>>>,
  /// The key each task had before it was uploaded (*was KON-12*), which the search finds.
  pub former_keys: Arc<std::collections::HashMap<TaskId, String>>,
  /// The project is shared on a server, which keeps no *Don't merge automatically* (ORC-69).
  pub shared: bool,
  pub pipelines: Arc<Vec<PipelineChoice>>,
  /// The project's default pipeline (Pipelines › *Make default*), which New task starts with.
  pub default_pipeline: Option<ResourceId>,
  /// The project's limits, whose default run budget and time limit New task starts with.
  pub limits: Arc<ProjectLimits>,
  /// Shared tasks whose pipeline this computer lacks, with the pipeline's slug.
  pub missing_pipelines: Arc<std::collections::HashMap<TaskId, String>>,
  /// Why each task of a reviewed group that holds its work waits.
  pub group_holds: Arc<BTreeMap<TaskId, GroupHold>>,
  /// The tasks of reviewed groups, which a group starts in its turn unless they are Canceled.
  pub grouped: Arc<std::collections::BTreeSet<TaskId>>,
  /// Where a person holds new starts.
  pub queue: Arc<super::hold::HeldQueue>,
  /// Tool requests awaiting a person's decision anywhere in this project, oldest first.
  pub pending_tool_approvals: Arc<Vec<ShownApproval>>,
  /// Tool requests auto mode's reviewer is judging: they wait without asking the person (no dialog, no
  /// *Needs you*) until the review ends, and only their card says so.
  pub reviewing_tool_approvals: Arc<Vec<ShownApproval>>,
  /// Questions with suggested answers waiting for the person anywhere in this project.
  pub asked: Arc<Vec<AskedQuestions>>,
  /// When each memory record of the project and of its tasks that waits for a person's approval was written.
  pub proposed_memory: Arc<Vec<Timestamp>>,
  /// The runs in the project's merge queue, those that left it and wait for a person and those a person took out of
  /// it, in the queue's order; not the merged ones (`crate::merge_queue`).
  pub merge_queue: Arc<Vec<orchester_core::merge_queue::QueueEntry>>,
  /// The phase of each task that has a run, from its latest run, or that it cannot be read.
  pub phases: Arc<Phases>,
  /// The tasks a run of which has not ended, as the store decides it (`active_task_run`): waiting for the person,
  /// stopped at a limit, paused, interrupted with its worktree kept, or unreadable. No runner changes it.
  pub active: Arc<std::collections::BTreeSet<TaskId>>,
  /// How many runs of each task were started and can be read.
  pub runs: Arc<BTreeMap<TaskId, usize>>,
  /// Runs recorded as running that lost their agent because no runner is connected; 0 while
  /// one is ([`Self::without_runner`]).
  pub lost_to_runner: usize,
  /// The active runs that auto-approve their agents' requests, oldest task first.
  pub auto_approving: Arc<Vec<AutoApprovingRun>>,
  /// Who works on the running runs, when the merged ones merged and the groups' order ([`super::at_work`]).
  pub run_facts: Arc<super::at_work::RunFacts>,
  /// What answers sent from this window answer, which no longer waits ([`Self::with_answered`]).
  pub answered: Arc<Vec<super::answering::Answered>>,
  /// What waits for the person, once asked for ([`Self::waiting`]).
  pub waiting_once: WaitingOnce,
}

/// An active run that auto-approves, as the status bar names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoApprovingRun {
  pub run: RunId,
  pub number: RunNumber,
  pub task: TaskId,
  pub key: Arc<str>,
}

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
      row.readiness = super::list::ReadinessKind::NotShared;
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
      row.readiness = super::list::ReadinessKind::Blocked;
    } else if let Some(reason) = row.organization.blocked_reason() {
      row.detail = UiMessage::new("tasks.blocked.informational").with("reason", reason.to_owned());
      row.readiness = super::list::ReadinessKind::Blocked;
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
      .filter(|(id, latest)| tasks.contains(id) && latest.phase().is_none_or(TaskPhase::run_not_ended))
      .count()
  }

  /// The *Not shared* tasks in place of the list's, for the list's *Not shared* filter (10.23).
  #[must_use]
  pub fn not_shared_view(&self) -> Self {
    Self {
      tasks: self.not_shared.clone(),
      waiting_once: WaitingOnce::default(),
      ..self.clone()
    }
  }

  /// Reads the tasks of the project at `repo_root`; git, through `programs`, tells the runs a person merged
  /// outside Orchester ([`super::delivery`]).
  pub fn load(repo_root: &Path, programs: &Programs) -> Result<Self, StoreError> {
    Self::read(&ProjectDb::open(repo_root)?, repo_root, Some(programs))
  }

  /// [`Self::load`] from `db`, the project database of `repo_root`, opened already: All projects reads a
  /// project's tasks without migrating its data. Without `programs` no git runs: a run merged outside Orchester
  /// reads as its stored phase.
  pub fn read(db: &ProjectDb, repo_root: &Path, programs: Option<&Programs>) -> Result<Self, StoreError> {
    let tasks = db.tasks()?;
    let (not_shared, kept_tags) = kept(db)?;
    let under_review = db.reviewing_tool_approvals()?;
    let (reviewing, pending): (Vec<_>, Vec<_>) = approvals::pending(db, repo_root)?
      .into_iter()
      .partition(|shown| under_review.contains(&shown.approval.id));
    let asked = db.waiting_questions()?;
    let asking: Vec<RunId> = asked.iter().map(|asked| asked.request.run()).collect();
    let (mut group_holds, grouped, previous_in_group) = group_holds(db)?;
    let mut unmerged = Unmerged::default();
    let LatestRuns {
      mut phases,
      counts,
      auto_approving,
      mut facts,
    } = latest_runs(
      db,
      tasks.iter().chain(&not_shared),
      (&pending, &asking),
      (&mut group_holds, &mut unmerged),
    )?;
    facts.previous_in_group = previous_in_group;
    if let Some(programs) = programs {
      unmerged.settle(programs, repo_root, &mut phases);
    }
    // Only a task with a run can have one that has not ended.
    let active = phases
      .keys()
      .filter_map(|task| db.active_task_run(*task).map(|run| run.map(|_| *task)).transpose())
      .collect::<Result<_, _>>()?;
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
      modules: Arc::new(db.modules()?),
      blocking: Arc::new(
        tasks
          .iter()
          .map(|task| db.unfinished_blocking_tasks(task.id()).map(|links| (task.id(), links)))
          .collect::<Result<_, _>>()?,
      ),
      proposed_memory: Arc::new(proposed_memory(db)?),
      merge_queue: Arc::new(
        db.merge_queue()?
          .into_iter()
          .filter(|entry| !matches!(entry.state, orchester_core::merge_queue::QueueState::Merged(_)))
          .collect(),
      ),
      tasks: Arc::new(tasks),
      not_shared: Arc::new(not_shared),
      kept_tags: Arc::new(kept_tags),
      former_keys: Arc::new(db.former_keys()?),
      shared: db.sharing()?.is_some(),
      group_holds: Arc::new(group_holds),
      queue: Arc::new(super::hold::held_in(db)?),
      grouped: Arc::new(grouped),
      pipelines: Arc::new(published_pipelines(db)?),
      default_pipeline: db.default_pipeline()?,
      limits: Arc::new(db.project_limits()?),
      missing_pipelines: Arc::new(db.missing_pipelines()?),
      pending_tool_approvals: Arc::new(pending),
      reviewing_tool_approvals: Arc::new(reviewing),
      asked: Arc::new(asked),
      phases: Arc::new(phases),
      active: Arc::new(active),
      runs: Arc::new(counts),
      lost_to_runner: 0,
      auto_approving: Arc::new(auto_approving),
      run_facts: Arc::new(facts),
      answered: Arc::default(),
      waiting_once: WaitingOnce::default(),
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
      waiting_once: WaitingOnce::default(),
      ..self.clone()
    }
  }
}

/// What one reading finds of the tasks' runs: the latest run of each task that has one, how many runs of each
/// can be read, the active runs that auto-approve, who works on the running ones and when the merged ones merged.
struct LatestRuns {
  phases: Phases,
  counts: BTreeMap<TaskId, usize>,
  auto_approving: Vec<AutoApprovingRun>,
  facts: super::at_work::RunFacts,
}

/// The latest run of each of `tasks`, a run waiting in its turn on one of the `pending` tool requests or on
/// a question of the `asking` runs waiting for a person. A task whose latest run is no group's leaves
/// `group_holds`; a latest run that succeeded and was not merged goes to `unmerged`.
fn latest_runs<'a>(
  db: &ProjectDb,
  tasks: impl Iterator<Item = &'a Task>,
  (pending, asking): (&[ShownApproval], &[RunId]),
  (group_holds, unmerged): (&mut BTreeMap<TaskId, GroupHold>, &mut Unmerged),
) -> Result<LatestRuns, StoreError> {
  let mut found = LatestRuns {
    phases: Phases::new(),
    counts: BTreeMap::new(),
    auto_approving: Vec::new(),
    facts: super::at_work::RunFacts::default(),
  };
  for task in tasks {
    let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    found
      .counts
      .insert(task.id(), runs.iter().filter(|run| run.is_ok()).count());
    let latest = match runs.last() {
      Some(Ok(latest)) => {
        if db.task_group_for_run(latest).map_err(run_store_error)?.is_none() {
          group_holds.remove(&task.id());
        }
        let (mut shown, agent) = latest_run_and_agent(db, task, latest)?;
        // A run whose agent waits for a tool decision or an answer in its turn waits for a person, as one at
        // a plan does.
        if shown.phase == TaskPhase::Running
          && (pending.iter().any(|shown| shown.approval.run == latest.id()) || asking.contains(&latest.id()))
        {
          shown.phase = TaskPhase::Waiting;
        }
        if shown.phase == TaskPhase::Succeeded {
          unmerged.add(task.id(), latest);
        }
        found.note(db, (latest, &shown), agent)?;
        if run_approvals::counts_as_auto_approving(db, task, latest)? {
          found.auto_approving.push(AutoApprovingRun {
            run: latest.id(),
            number: latest.number(),
            task: task.id(),
            key: task.key().to_string().into(),
          });
        }
        Latest::Run(Box::new(shown))
      }
      Some(Err(unreadable)) => Latest::Unreadable(Unreadable::of(unreadable)),
      None => continue,
    };
    found.phases.insert(task.id(), latest);
  }
  Ok(found)
}

impl LatestRuns {
  /// Keeps who works on `latest` while it runs (`agent`, its step's agent as reading it found it), and when it merged
  /// once it was merged in Orchester.
  fn note(
    &mut self,
    db: &ProjectDb,
    (latest, shown): (&Run, &LatestRun),
    agent: Option<super::at_work::FoundAgent>,
  ) -> Result<(), StoreError> {
    match shown.phase {
      TaskPhase::Running => {
        let at_work = super::at_work::read(db, latest, agent)?;
        self.facts.at_work.insert(latest.id(), at_work);
      }
      TaskPhase::Merged => {
        if let Some(merge) = db.run_merge(latest.id())? {
          self.facts.merged_at.insert(latest.id(), merge.merged_at);
        }
      }
      _ => {}
    }
    Ok(())
  }
}

/// The reviewed groups as a reading keeps them: why each task of a group that holds its work waits, every grouped
/// task, and the task before each in its group's order.
type Groups = (
  BTreeMap<TaskId, GroupHold>,
  std::collections::BTreeSet<TaskId>,
  BTreeMap<TaskId, TaskId>,
);

fn group_holds(db: &ProjectDb) -> Result<Groups, StoreError> {
  let mut holds = BTreeMap::new();
  let mut grouped = std::collections::BTreeSet::new();
  let mut previous = BTreeMap::new();
  for group in db.task_groups()? {
    grouped.extend(group.tasks().iter().copied());
    previous.extend(group.tasks().windows(2).map(|pair| (pair[1], pair[0])));
    if let Some(hold) = db.task_group_hold(group.key())? {
      for task in group.tasks() {
        holds.insert(*task, hold.clone());
      }
    }
  }
  Ok((holds, grouped, previous))
}

/// When each record of the project's memory and of its tasks' memories that waits for a person's approval was
/// written, in one listing.
fn proposed_memory(db: &ProjectDb) -> Result<Vec<Timestamp>, StoreError> {
  let proposed = MemoryFilter {
    scope: ScopeFilter::All,
    status: Some(MemoryStatus::Proposed),
    pinned_only: false,
  };
  Ok(db.memories(&proposed)?.iter().map(Memory::created_at).collect())
}

/// `latest`, a run of `task`, with when it last changed, what it waits for, who runs its step's agent and, once it was
/// interrupted, whether a person removed its worktree so that its task runs again after it (ORC-63).
pub(super) fn latest_run(db: &ProjectDb, task: &Task, latest: &Run) -> Result<LatestRun, StoreError> {
  latest_run_and_agent(db, task, latest).map(|(shown, _)| shown)
}

/// [`latest_run`], with its step's agent as it was found, so that a running run's facts don't look it up again.
fn latest_run_and_agent(
  db: &ProjectDb,
  task: &Task,
  latest: &Run,
) -> Result<(LatestRun, Option<super::at_work::FoundAgent>), StoreError> {
  let updated_at = db
    .events(latest.id())
    .map_err(run_store_error)?
    .last()
    .map_or_else(|| latest.created_at(), RunEvent::at);
  let awaited = awaited(db, latest)?;
  let merge = db.run_merge(latest.id())?;
  let mut shown = LatestRun::of(task, (latest, merge.as_ref()), updated_at, awaited);
  let agent = super::at_work::step_agent(db, latest, shown.step.as_ref())?;
  shown.agent = agent.as_ref().map(|found| found.provider);
  match latest.status() {
    RunStatus::Interrupted(_) => {
      let removed = db.worktree_removal(latest.id())?.is_some();
      shown.ended_for_run_again = can_run_again_after(latest, removed);
    }
    RunStatus::Succeeded => shown.completed = db.run_completion(latest.id())?.is_some(),
    _ => {}
  }
  Ok((shown, agent))
}

/// Each step of `run` that waits on a decision on its plan or for an answer to its question, with the
/// agent role of the step that proposed or asked it; only such a run needs its plans or its conversation
/// and its profiles read here. A plan that cannot be read is left out.
fn awaited(db: &ProjectDb, run: &Run) -> Result<Vec<AwaitedStep>, StoreError> {
  let (plans, answers) = (run.awaiting_plan(), run.awaiting_answer());
  if plans.is_empty() && answers.is_empty() {
    return Ok(Vec::new());
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
  let mut waiting = Vec::new();
  if !plans.is_empty() {
    let stored = db.plans(run.id()).map_err(run_store_error)?;
    waiting.extend(plans.into_iter().filter_map(|execution| {
      Some(AwaitedStep {
        execution,
        step: CurrentStep::of_execution(run, execution),
        wait: StepWait::Plan(AwaitedPlan::of(run, &stored, execution, role)?),
      })
    }));
  }
  if !answers.is_empty() {
    let messages = db.messages(run.task())?;
    waiting.extend(answers.into_iter().map(|execution| AwaitedStep {
      execution,
      step: CurrentStep::of_execution(run, execution),
      wait: StepWait::Answer(awaited_question(run, &messages, execution, role)),
    }));
  }
  Ok(waiting)
}

/// The question `execution`, waiting for an answer, asked, among `messages`, and who asks.
fn awaited_question(
  run: &Run,
  messages: &[Message],
  execution: StepExecutionId,
  role: impl Fn(&StepKey) -> Option<Arc<str>>,
) -> Option<AwaitedQuestion> {
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
    delivers: version.body().delivers(),
  }
}

fn step_choices(steps: &[Step], assignables: &Assignables) -> Vec<PipelineStepChoice> {
  steps
    .iter()
    .map(|step| PipelineStepChoice {
      name: step.name.as_str().into(),
      executor: describe::executor_line(step, assignables),
      tool: matches!(step.kind, orchester_core::resource::StepKind::Tool { .. }),
      pull_request: step.key.as_str() == orchester_core::resource::StarterStep::CreatePullRequest.key(),
      approval_gate: step.approval_gate,
    })
    .collect()
}
