//! The task list (02.03 `J9467`) and the overview's task card (02.08 `Twtqc`): tabs, which
//! tasks each shows, and what a row says about a task.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  connection::AgentProvider,
  id::{ModuleId, ResourceId, TaskId},
  integration::RunMerge,
  resource::{Deliverable, VersionNumber, WithoutCommits},
  run::{PauseReason, PinnedVersion, Run, RunLimit, RunNumber, RunStatus, TaskPhase, run_delivers},
  task::{MissingDetail, Task, TaskStatus},
  time::Timestamp,
  tokens::TokenBudget,
};

pub use super::{
  awaited::{AwaitedDecision, AwaitedPlan, AwaitedQuestion, AwaitedStep, DecisionWhy, StepWait},
  standing::{CurrentStep, ReadinessKind, RunStage},
};
use crate::{
  display,
  messages::Message,
  tasks::{
    data::{PipelineChoice, ProjectTasks},
    unreadable::Unreadable,
  },
};

/// Workflow tabs, and Needs you: the tasks something of waits for the person
/// ([`ProjectTasks::waiting`](super::data::ProjectTasks::waiting)), whatever their workflow status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskTab {
  All,
  NeedsYou,
  Backlog,
  Todo,
  InProgress,
  InReview,
  Done,
}
impl TaskTab {
  pub const ALL: [Self; 7] = [
    Self::All,
    Self::NeedsYou,
    Self::Backlog,
    Self::Todo,
    Self::InProgress,
    Self::InReview,
    Self::Done,
  ];
  pub const fn label_key(self) -> &'static str {
    match self {
      Self::All => "tasks.tabs.all",
      Self::NeedsYou => "tasks.tabs.needsYou",
      Self::Backlog => "tasks.workflow.backlog",
      Self::Todo => "tasks.workflow.todo",
      Self::InProgress => "tasks.workflow.inProgress",
      Self::InReview => "tasks.workflow.inReview",
      Self::Done => "tasks.workflow.done",
    }
  }
  /// How many the tab holds of `data`'s tasks, as [`rows`] lists them: Needs you the tasks something of waits
  /// for ([`ProjectTasks::waiting_tasks`]). What waits and is no task's is not among them; the tab names it
  /// over its list instead ([`super::attention::outside_tasks`]).
  pub fn count(self, data: &ProjectTasks) -> usize {
    if self == Self::NeedsYou {
      return data.waiting_tasks().len();
    }
    data.tasks.iter().filter(|task| self.includes(task)).count()
  }

  pub fn includes(self, task: &Task) -> bool {
    use orchester_core::task::organization::WorkflowStatus;
    match self {
      Self::All | Self::NeedsYou => true,
      Self::Backlog => task.organization().status() == WorkflowStatus::Backlog,
      Self::Todo => task.organization().status() == WorkflowStatus::Todo,
      Self::InProgress => task.organization().status() == WorkflowStatus::InProgress,
      Self::InReview => task.organization().status() == WorkflowStatus::InReview,
      Self::Done => task.organization().status() == WorkflowStatus::Done,
    }
  }
}

/// Where a task's latest run stands, for its row's status and its dependency / readiness cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestRun {
  pub id: orchester_core::id::RunId,
  /// The configuration the latest readable run actually executes.
  pub pipeline: PinnedVersion,
  pub phase: TaskPhase,
  /// The run's number in the project, e.g. 128 for "Run #128".
  pub number: RunNumber,
  /// When the run last changed: its last recorded event.
  pub updated_at: Timestamp,
  /// Stopped by the project's daily budget rather than its own limit.
  pub daily_budget: bool,
  /// The limit it stopped at, while it waits at one.
  pub limit: Option<RunLimit>,
  /// The token budget it stopped at, when its own token limit stopped it.
  pub token_limit: Option<u64>,
  /// Running, with a pause requested: it pauses at its next checkpoint.
  pub pausing: bool,
  /// Its last checkpoint, as people read a revision.
  pub checkpoint: Option<Arc<str>>,
  /// The plan it waits on a decision for, if it does; the first in pipeline order when several do.
  pub plan: Option<AwaitedPlan>,
  /// The branch its work was merged into, once it was, in Orchester or by a person
  /// ([`super::delivery`]).
  pub merged_into: Option<Arc<str>>,
  /// It passed every step and committed nothing, so there is nothing to merge ([`super::delivery`]).
  pub nothing_to_merge: bool,
  /// The question it waits for an answer to, if it does; the first that can be read when several do.
  pub question: Option<AwaitedQuestion>,
  /// Each of its steps that waits on a plan or for an answer, plans first, each in pipeline order.
  pub waiting: Vec<AwaitedStep>,
  /// The decision it waits for after its retries ran out or its step broke again, if it does.
  pub decision: Option<AwaitedDecision>,
  /// The step its agent is on, or the one it stopped at.
  pub step: Option<CurrentStep>,
  /// It reads interrupted only because no runner looks after it ([`LatestRun::without_runner`]); a runner
  /// records it once one does.
  pub lost_runner: bool,
  /// Its pipeline ends in a Deliver step: once every step passed, the project's merge queue delivers it
  /// (`crate::merge_queue`), not the person.
  pub delivers: bool,
  /// What it delivers (ORC-93): with [`Self::nothing_to_merge`], whether it is completed or flagged.
  pub deliverable: Deliverable,
  /// Who runs the agent of [`Self::step`]: the provider of its profile's connection; `None` for a tool step, before
  /// the first step, or when the profile or its connection cannot be found ([`super::data`] reads it).
  pub agent: Option<AgentProvider>,
}

impl LatestRun {
  /// `run`, the latest run of `task`, merged as `merge` records, whose last event was recorded at
  /// `updated_at`, with the steps that wait on a plan or a question, if any.
  pub fn of(
    task: &Task,
    (run, merge): (&Run, Option<&RunMerge>),
    updated_at: Timestamp,
    waiting: Vec<AwaitedStep>,
  ) -> Self {
    let plan = waiting.iter().find_map(|step| match &step.wait {
      StepWait::Plan(plan) => Some(plan.clone()),
      StepWait::Answer(_) => None,
    });
    let question = waiting.iter().find_map(|step| match &step.wait {
      StepWait::Answer(question) => question.clone(),
      StepWait::Plan(_) => None,
    });
    Self {
      id: run.id(),
      pipeline: run.snapshot().pipeline(),
      phase: TaskPhase::of(task, Some(run), merge),
      merged_into: merge.map(|merge| merge.into.as_str().into()),
      nothing_to_merge: false,
      number: run.number(),
      updated_at,
      daily_budget: run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)),
      limit: match run.status() {
        RunStatus::Paused(PauseReason::LimitReached(limit)) => Some(*limit),
        _ => None,
      },
      token_limit: (run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens)))
        .then(|| run.budget().map(TokenBudget::get))
        .flatten(),
      pausing: run.status() == &RunStatus::Running && run.pause_requested(),
      checkpoint: run
        .last_checkpoint()
        .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).into()),
      plan,
      question,
      waiting,
      decision: AwaitedDecision::of(run),
      step: CurrentStep::of(run),
      lost_runner: false,
      delivers: run.snapshot().body().deliver().is_some(),
      deliverable: run_delivers(task, run),
      agent: None,
    }
  }

  /// The run while no runner is connected: a running run is interrupted, and a requested pause
  /// cannot settle.
  #[must_use]
  pub fn without_runner(&self) -> Self {
    let phase = self.phase.without_runner();
    Self {
      phase,
      pausing: self.pausing && phase == TaskPhase::Running,
      lost_runner: self.lost_runner || phase != self.phase,
      ..self.clone()
    }
  }

  /// Whether Run again may follow the run, as the runner's check of core's `run_again` reads it: it failed or was
  /// cancelled, or it passed every step and committed nothing although its pipeline expects changes (ORC-93). An
  /// interrupted run has not ended on the board, so its lost worktree is not asked about here.
  pub fn runs_again(&self) -> bool {
    match self.phase {
      TaskPhase::Failed | TaskPhase::Cancelled => true,
      TaskPhase::Succeeded => self.nothing_to_merge && self.deliverable.without_commits() == WithoutCommits::Flagged,
      _ => false,
    }
  }

  /// What the run means for the task now: "Paused at 4b92ad0", "Last checkpoint 1b8e5c2".
  pub fn message(&self) -> Message {
    let with_checkpoint = |key: &'static str, without: &'static str| match &self.checkpoint {
      Some(checkpoint) => Message::new(key).with("checkpoint", checkpoint.to_string()),
      None => Message::new(without),
    };
    match self.phase {
      TaskPhase::Draft => Message::new("tasks.readiness.ready"),
      TaskPhase::Queued => Message::new("tasks.progress.queued"),
      TaskPhase::Running if self.pausing => Message::new("tasks.progress.pausing"),
      TaskPhase::Running => Message::new("tasks.progress.running"),
      TaskPhase::Waiting | TaskPhase::Stuck => Message::new("tasks.progress.waiting"),
      TaskPhase::Paused => with_checkpoint("tasks.progress.paused", "tasks.progress.pausedNoCheckpoint"),
      TaskPhase::StoppedAtLimit if self.daily_budget => Message::new("tasks.progress.stoppedDaily"),
      TaskPhase::StoppedAtLimit => Message::new("tasks.progress.stoppedAtLimit"),
      TaskPhase::Interrupted => with_checkpoint("tasks.progress.interrupted", "tasks.progress.interruptedNoCheckpoint"),
      TaskPhase::Succeeded if self.nothing_to_merge => match self.deliverable.without_commits() {
        WithoutCommits::Completes => Message::new("tasks.progress.reportReady"),
        WithoutCommits::Flagged => Message::new("tasks.progress.committedNothing"),
      },
      TaskPhase::Succeeded => Message::new("tasks.progress.succeeded"),
      TaskPhase::Merged => {
        Message::new("tasks.progress.merged").with("branch", self.merged_into.as_deref().unwrap_or_default().to_owned())
      }
      TaskPhase::Failed => Message::new("tasks.progress.failed"),
      TaskPhase::Cancelled => Message::new("tasks.progress.cancelled"),
    }
  }
}

/// A task's latest run: where it stands, or that it cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Latest {
  Run(Box<LatestRun>),
  Unreadable(Unreadable),
}

impl Latest {
  /// Where the task stands; `None` when its latest run cannot be read.
  pub fn phase(&self) -> Option<TaskPhase> {
    match self {
      Self::Run(latest) => Some(latest.phase),
      Self::Unreadable(_) => None,
    }
  }

  pub fn run(&self) -> Option<&LatestRun> {
    match self {
      Self::Run(latest) => Some(latest),
      Self::Unreadable(_) => None,
    }
  }

  /// What the run means for the task now; "Run #9 could not be read".
  pub fn message(&self) -> Message {
    match self {
      Self::Run(latest) => latest.message(),
      Self::Unreadable(unreadable) => unreadable.title(),
    }
  }

  /// What [`Latest::message`] is about.
  pub const fn readiness(&self) -> ReadinessKind {
    match self {
      Self::Run(latest) if matches!(latest.phase, TaskPhase::Draft) => ReadinessKind::Ready,
      Self::Run(latest) => ReadinessKind::Run(latest.phase),
      Self::Unreadable(_) => ReadinessKind::Unreadable,
    }
  }

  /// The latest run while no runner is connected ([`LatestRun::without_runner`]); a run that
  /// cannot be read stays as it is.
  #[must_use]
  pub fn without_runner(&self) -> Self {
    match self {
      Self::Run(latest) => Self::Run(Box::new(latest.without_runner())),
      Self::Unreadable(_) => self.clone(),
    }
  }
}

/// How many tasks wait in the queue for their run to start (the status bar's Queued count); a closed task waits for
/// nothing.
pub fn queued(tasks: &[Task], phases: &Phases) -> usize {
  tasks
    .iter()
    .filter(|task| !task.organization().status().is_closed() && phase_of(task, phases) == Some(TaskPhase::Queued))
    .count()
}

/// What a task still needs before it can be queued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
  Ready,
  Missing(Vec<MissingDetail>),
}

impl Readiness {
  pub fn of(task: &Task) -> Self {
    let missing = task.details().missing();
    if missing.is_empty() {
      Self::Ready
    } else {
      Self::Missing(missing)
    }
  }

  pub const fn kind(&self) -> ReadinessKind {
    match self {
      Self::Ready => ReadinessKind::Ready,
      Self::Missing(_) => ReadinessKind::Missing,
    }
  }

  /// One detail is named; several are counted, so no list of names is assembled from parts.
  pub fn message(&self) -> Message {
    match self {
      Self::Ready => Message::new("tasks.readiness.ready"),
      Self::Missing(missing) => match missing.as_slice() {
        [single] => Message::new(missing_key(*single)),
        several => Message::new("tasks.readiness.missingCount").with("count", several.len().to_string()),
      },
    }
  }
}

pub const fn missing_key(detail: MissingDetail) -> &'static str {
  match detail {
    MissingDetail::Brief => "tasks.readiness.missingBrief",
    MissingDetail::AcceptanceCriteria => "tasks.readiness.missingCriteria",
    MissingDetail::Pipeline => "tasks.readiness.missingPipeline",
  }
}

/// The pipeline a task runs, as far as the project's published pipelines know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipelineLabel {
  NotChosen,
  Published {
    name: Arc<str>,
    version: VersionNumber,
  },
  /// The chosen pipeline has no published version in this project any more.
  Unavailable,
  /// A shared task's pipeline, by its slug, that this computer has no pipeline for.
  Missing {
    slug: Arc<str>,
  },
}

impl PipelineLabel {
  pub fn of(pipeline: Option<ResourceId>, published: &[PipelineChoice]) -> Self {
    let Some(pipeline) = pipeline else {
      return Self::NotChosen;
    };
    published
      .iter()
      .find(|choice| choice.id == pipeline)
      .map_or(Self::Unavailable, |choice| Self::Published {
        name: choice.name.clone(),
        version: choice.version,
      })
  }

  pub fn message(&self) -> Message {
    match self {
      Self::NotChosen => Message::new("tasks.pipeline.notChosen"),
      Self::Published { name, version } => Message::new("tasks.pipeline.published")
        .with("name", name.to_string())
        .with("version", version.to_string()),
      Self::Unavailable => Message::new("tasks.pipeline.unavailable"),
      Self::Missing { slug } => Message::new("tasks.pipeline.missing").with("slug", slug.to_string()),
    }
  }
}

/// One task as a list row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRow {
  pub organization: orchester_core::task::organization::TaskOrganization,
  pub tags: Vec<Arc<str>>,
  pub id: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub pipeline: PipelineLabel,
  /// Where the task stands, from its latest run; `None` when that run cannot be read.
  pub phase: Option<TaskPhase>,
  /// The dependency / readiness cell: what the task needs before it can be queued, or once it
  /// is queued or has run, where it stands.
  pub detail: Message,
  /// The plan its run waits on a decision for, if it does.
  pub plan: Option<AwaitedPlan>,
  /// The question its run waits for an answer to, if it does.
  pub question: Option<AwaitedQuestion>,
  /// The decision its run waits for after its retries, if it does.
  pub decision: Option<AwaitedDecision>,
  pub created_at: Timestamp,
  /// The key the task had before it was uploaded to a server project (*was KON-12*).
  pub former_key: Option<Arc<str>>,
  /// Kept *Not shared* when the project joined: read-only, shown behind the filter only.
  pub not_shared: bool,
  /// What `detail` is about, set wherever `detail` is chosen.
  pub readiness: ReadinessKind,
  /// The step its latest run is on, or stopped at.
  pub step: Option<CurrentStep>,
  /// Its latest run passed every step and committed nothing, so there is nothing to merge.
  pub nothing_to_merge: bool,
  /// Its story points, who set them and why; `None` while it has none.
  pub estimate: Option<orchester_core::task::estimate::StoryEstimate>,
  /// Whether a person submitted it for running: a draft, or queued.
  pub status: TaskStatus,
  /// It has a run, readable or not: it no longer goes back to the drafts.
  pub has_run: bool,
  /// Its latest run ended so that Run again may follow it, as core's `run_again` decides: it failed or was
  /// cancelled, or it passed every step and committed nothing although its pipeline expects changes (ORC-93). A run
  /// that succeeded with work to merge, or was merged, is never run again.
  pub runs_again: bool,
  /// A run of it has not ended, as the store decides it ([`ProjectTasks::active`]): only closing it, after its run is
  /// stopped, changes its workflow status by hand on the board.
  pub run_not_ended: bool,
  /// The module it belongs to, if any.
  pub module: Option<ModuleId>,
  /// Who runs the agent of its latest run's step ([`LatestRun::agent`]).
  pub agent: Option<AgentProvider>,
}

/// The latest run of each task that has one; a task without one is a draft or queued.
pub type Phases = HashMap<TaskId, Latest>;

/// Where `task` stands; `None` when its latest run cannot be read.
pub(super) fn phase_of(task: &Task, phases: &Phases) -> Option<TaskPhase> {
  phases
    .get(&task.id())
    .map_or_else(|| Some(TaskPhase::of(task, None, None)), Latest::phase)
}

impl TaskRow {
  pub fn new(task: &Task, published: &[PipelineChoice], latest: Option<&Latest>) -> Self {
    let phase = latest.map_or_else(|| Some(TaskPhase::of(task, None, None)), Latest::phase);
    let (detail, readiness) = match (latest, task.status()) {
      (Some(latest), _) => (latest.message(), latest.readiness()),
      (None, TaskStatus::Queued) => (
        Message::new("tasks.progress.queued"),
        ReadinessKind::Run(TaskPhase::Queued),
      ),
      (None, TaskStatus::Draft) => {
        let readiness = Readiness::of(task);
        (readiness.message(), readiness.kind())
      }
    };
    let has_run = latest.is_some();
    let latest = latest.and_then(Latest::run);
    let runs_again = latest.is_some_and(LatestRun::runs_again);
    Self {
      id: task.id(),
      organization: task.organization().clone(),
      tags: Vec::new(),
      key: task.key().to_string().into(),
      title: task.details().title.as_str().into(),
      pipeline: task_pipeline_label(task, published, latest),
      phase,
      detail,
      plan: latest.and_then(|latest| latest.plan.clone()),
      question: latest.and_then(|latest| latest.question.clone()),
      decision: latest.and_then(|latest| latest.decision.clone()),
      created_at: task.created_at(),
      former_key: None,
      not_shared: false,
      readiness,
      step: latest.and_then(|latest| latest.step.clone()),
      nothing_to_merge: latest.is_some_and(|latest| latest.nothing_to_merge),
      estimate: task.estimate().cloned(),
      status: task.status(),
      has_run,
      runs_again,
      run_not_ended: false,
      module: task.module(),
      agent: latest.and_then(|latest| latest.agent),
    }
  }
}

impl TaskRow {
  /// The step the row's run line names: the one its run is at work on, or the one it failed at
  /// (`Running · 02 Plan`, `Failed · 04 Run checks`).
  pub fn run_step(&self) -> Option<&CurrentStep> {
    matches!(self.phase, Some(TaskPhase::Running | TaskPhase::Failed))
      .then_some(self.step.as_ref())
      .flatten()
  }
}

fn task_pipeline_label(task: &Task, published: &[PipelineChoice], latest: Option<&LatestRun>) -> PipelineLabel {
  let pinned = latest
    .map(|run| (run.pipeline.resource, run.pipeline.number))
    .or_else(|| task.details().pipeline.zip(task.pipeline_version()));
  let id = pinned.map(|(id, _)| id).or(task.details().pipeline);
  let mut label = PipelineLabel::of(id, published);
  if let PipelineLabel::Published { version, .. } = &mut label
    && let Some((_, number)) = pinned
  {
    *version = number;
  }
  label
}

/// Rows of `tab` among `tasks`, tasks of `data`, in task number order.
pub fn rows(tasks: &[Task], data: &ProjectTasks, tab: TaskTab) -> Vec<TaskRow> {
  let waiting = (tab == TaskTab::NeedsYou).then(|| data.waiting_tasks());
  tasks
    .iter()
    .filter(|task| tab.includes(task) && waiting.as_ref().is_none_or(|waiting| waiting.contains(&task.id())))
    .map(|task| TaskRow {
      run_not_ended: data.active.contains(&task.id()),
      ..TaskRow::new(task, &data.pipelines, data.phases.get(&task.id()))
    })
    .collect()
}

/// The newest `limit` tasks, newest first, for the overview.
pub fn recent(tasks: &[Task], published: &[PipelineChoice], phases: &Phases, limit: usize) -> Vec<TaskRow> {
  tasks
    .iter()
    .rev()
    .take(limit)
    .map(|task| TaskRow::new(task, published, phases.get(&task.id())))
    .collect()
}

/// Counts under the task list: drafts that could be queued and drafts still missing details.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DraftSummary {
  pub ready: usize,
  pub incomplete: usize,
}

impl DraftSummary {
  pub fn of(tasks: &[Task]) -> Self {
    tasks
      .iter()
      .filter(|task| task.status() == TaskStatus::Draft)
      .fold(Self::default(), |summary, task| {
        if task.details().missing().is_empty() {
          Self {
            ready: summary.ready + 1,
            ..summary
          }
        } else {
          Self {
            incomplete: summary.incomplete + 1,
            ..summary
          }
        }
      })
  }
}

/// Under the task list (`Bpth7`): how many tasks there are and how many runs were started.
/// The table's count (`Bpth7`): the rows shown and their runs, or how many rows match while a
/// search or filter narrows them (`l7kBSq`).
pub fn footer(tasks: usize, runs: usize, filtered: bool) -> Message {
  if filtered {
    let key = if tasks == 1 {
      "tasks.footer.matches.one"
    } else {
      "tasks.footer.matches.other"
    };
    return Message::new(key).with("count", tasks.to_string());
  }
  let key = match (tasks == 1, runs) {
    (true, 0) => "tasks.footer.noRuns.one",
    (false, 0) => "tasks.footer.noRuns.other",
    (true, 1) => "tasks.footer.oneRun.one",
    (false, 1) => "tasks.footer.oneRun.other",
    (true, _) => "tasks.footer.runs.one",
    (false, _) => "tasks.footer.runs.other",
  };
  Message::new(key)
    .with("count", tasks.to_string())
    .with("runs", runs.to_string())
}
