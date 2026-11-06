//! Reading and saving tasks through the project database. Every function blocks on SQLite;
//! the UI calls them off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  limits::ProjectLimits,
  memory::{Memory, MemoryFilter, MemoryStatus, MemoryStore, ScopeFilter},
  question::AskedQuestions,
  resource::{Pipeline, ResourceKind, Step, VersionNumber},
  run::{RunNumber, TaskPhase},
  store::{ResourceStore, SettingsStore, TaskStore},
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
  latest_runs::{LatestRuns, latest_runs},
  list::Phases,
  needs_you::WaitingOnce,
  project_runs::ProjectRuns,
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
  /// reads as its stored phase. The tasks' runs are read for the whole project at once ([`ProjectRuns::read`]).
  pub fn read(db: &ProjectDb, repo_root: &Path, programs: Option<&Programs>) -> Result<Self, StoreError> {
    Self::read_with(db, (repo_root, programs), &ProjectRuns::read(db)?)
  }

  /// [`Self::read`] with the tasks' runs read task by task, as it read them before ORC-742, for the tests that
  /// compare the two.
  #[cfg(feature = "test-support")]
  pub fn read_task_by_task(db: &ProjectDb, repo_root: &Path, programs: Option<&Programs>) -> Result<Self, StoreError> {
    let runs = ProjectRuns::read_per_task(db, &db.tasks()?, &db.not_shared_tasks()?)?;
    Self::read_with(db, (repo_root, programs), &runs)
  }

  /// [`Self::read`] with the project's `runs` read already.
  fn read_with(
    db: &ProjectDb,
    (repo_root, programs): (&Path, Option<&Programs>),
    runs: &ProjectRuns,
  ) -> Result<Self, StoreError> {
    let tasks = db.tasks()?;
    let not_shared = db.not_shared_tasks()?;
    let kept_tags = not_shared
      .iter()
      .filter_map(|task| Some((task.id(), runs.kept_tags(task.id())?.clone())))
      .collect();
    let (reviewing, pending): (Vec<_>, Vec<_>) = approvals::pending(db, repo_root)?
      .into_iter()
      .partition(|shown| shown.reviewing);
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
      (tasks.iter().chain(&not_shared), runs),
      (&pending, &asking),
      (&mut group_holds, &mut unmerged),
    )?;
    facts.previous_in_group = previous_in_group;
    if let Some(programs) = programs {
      unmerged.settle(programs, repo_root, &mut phases);
    }
    // Only a task with a run can have one that has not ended.
    let active = phases.keys().copied().filter(|task| runs.is_active(*task)).collect();
    Ok(Self {
      updated: Arc::new(
        tasks
          .iter()
          .map(|task| (task.id(), runs.organization_updated(task)))
          .collect(),
      ),
      tags: Arc::new(db.project_tags()?),
      modules: Arc::new(db.modules()?),
      blocking: Arc::new(tasks.iter().map(|task| (task.id(), runs.blocking(task.id()))).collect()),
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

/// Pipelines with a published version the project offers (in a team project only the team's,
/// `crate::pipelines::offered`), ordered by slug. Drafts that were never published cannot be run,
/// so they are not offered.
pub fn published_pipelines(store: &ProjectDb) -> Result<Vec<PipelineChoice>, StoreError> {
  let mut choices = Vec::new();
  let assignables = Assignables::load(store)?;
  let offered = crate::pipelines::offered::Offered::of(store)?;
  for resource in store.resources(ResourceKind::Pipeline)? {
    let Some(version) = store.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    if !offered.offers(store, resource.id(), true)? {
      continue;
    }
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
