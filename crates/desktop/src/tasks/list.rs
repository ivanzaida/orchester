//! The task list (02.03 `J9467`) and the overview's task card (02.08 `Twtqc`): tabs, which
//! tasks each shows, and what a row says about a task.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  id::{ResourceId, TaskId},
  integration::RunMerge,
  resource::{StepKey, VersionNumber},
  run::{PauseReason, PinnedVersion, Plan, PlanVersion, Run, RunLimit, RunNumber, RunStatus, Stuck, TaskPhase},
  task::{MissingDetail, Task, TaskStatus},
  time::Timestamp,
};

use crate::{
  display,
  messages::Message,
  tasks::{data::PipelineChoice, unreadable::Unreadable},
};

/// Workflow tabs; Needs you remains a run-state view.
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
impl lurq::app::component::DevtoolsInspectable for TaskTab {}
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
  /// Running, with a pause requested: it pauses at its next checkpoint.
  pub pausing: bool,
  /// Its last checkpoint, as people read a revision.
  pub checkpoint: Option<Arc<str>>,
  /// The plan it waits on a decision for, if it does.
  pub plan: Option<AwaitedPlan>,
  /// The branch its work was merged into, once it was.
  pub merged_into: Option<Arc<str>>,
  /// The question it waits for an answer to, if it does.
  pub question: Option<AwaitedQuestion>,
  /// The decision it waits for after its retries ran out or its step broke again, if it does.
  pub decision: Option<AwaitedDecision>,
}

/// What a run waiting for a person's decision stopped at (02.B6 `Np9kc`): the step, by name, and
/// whether its retries ran out (how many it took) or it broke again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedDecision {
  pub step: Arc<str>,
  pub retries: u32,
  pub broke: bool,
}

impl AwaitedDecision {
  /// What `run` waits for a decision on, if it does.
  pub fn of(run: &Run) -> Option<Self> {
    let RunStatus::WaitingForPerson(stuck) = run.status() else {
      return None;
    };
    let step = stuck.step();
    Some(Self {
      step: run
        .snapshot()
        .body()
        .step(step)
        .map_or_else(|| step.as_str().into(), |pinned| pinned.name.as_str().into()),
      retries: run.automatic_retries_taken(step),
      broke: matches!(stuck, Stuck::BrokeAgain(_)),
    })
  }

  /// "Review still fails after 2 retries", or "Implement stopped again without a result".
  pub fn title(&self) -> Message {
    let key = match (self.broke, self.retries) {
      (true, _) => "runs.retryLimit.brokeTitle",
      (false, 1) => "runs.retryLimit.title.one",
      (false, _) => "runs.retryLimit.title.other",
    };
    Message::new(key)
      .with("step", self.step.to_string())
      .with("retries", self.retries.to_string())
  }
}

/// A question an agent asked, which its run waits for an answer to: who asks (the agent role of
/// its step, the step's name when its profile version is gone) and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedQuestion {
  pub agent: Arc<str>,
  pub text: Arc<str>,
}

/// What a run waits for from a person, if anything: a plan decision or an answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Awaited {
  pub plan: Option<AwaitedPlan>,
  pub question: Option<AwaitedQuestion>,
}

/// A plan a run waits on a decision for (02.11): its version and who proposed it, the agent role
/// of its step (the step's name when its profile version is gone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitedPlan {
  pub version: PlanVersion,
  pub agent: Arc<str>,
}

impl AwaitedPlan {
  /// The plan `run` waits on, among its `plans`, proposed by `agent`'s step; the first in pipeline
  /// order when several steps' plans wait.
  pub fn of(run: &Run, plans: &[Plan], agent: impl Fn(&StepKey) -> Option<Arc<str>>) -> Option<Self> {
    let execution = *run.awaiting_plan().first()?;
    let plan = Plan::latest_of(plans, execution)?;
    let step = run.snapshot().body().step(&plan.step)?;
    Some(Self {
      version: plan.version,
      agent: agent(&plan.step).unwrap_or_else(|| step.name.as_str().into()),
    })
  }
}

impl LatestRun {
  /// `run`, the latest run of `task`, merged as `merge` records, whose last event was recorded at
  /// `updated_at`, with the plan or the question it waits on, if any.
  pub fn of(task: &Task, (run, merge): (&Run, Option<&RunMerge>), updated_at: Timestamp, awaited: Awaited) -> Self {
    Self {
      id: run.id(),
      pipeline: run.snapshot().pipeline(),
      phase: TaskPhase::of(task, Some(run), merge),
      merged_into: merge.map(|merge| merge.into.as_str().into()),
      number: run.number(),
      updated_at,
      daily_budget: run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)),
      pausing: run.status() == &RunStatus::Running && run.pause_requested(),
      checkpoint: run
        .last_checkpoint()
        .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).into()),
      plan: awaited.plan,
      question: awaited.question,
      decision: AwaitedDecision::of(run),
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
      ..self.clone()
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
  Run(LatestRun),
  Unreadable(Unreadable),
}

impl Latest {
  /// Where the task stands; `None` when its latest run cannot be read.
  pub const fn phase(&self) -> Option<TaskPhase> {
    match self {
      Self::Run(latest) => Some(latest.phase),
      Self::Unreadable(_) => None,
    }
  }

  pub const fn run(&self) -> Option<&LatestRun> {
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

  /// The latest run while no runner is connected ([`LatestRun::without_runner`]); a run that
  /// cannot be read stays as it is.
  #[must_use]
  pub fn without_runner(&self) -> Self {
    match self {
      Self::Run(latest) => Self::Run(latest.without_runner()),
      Self::Unreadable(_) => self.clone(),
    }
  }
}

/// How many tasks wait in the queue for their run to start (the status bar's Queued count).
pub fn queued(tasks: &[Task], phases: &Phases) -> usize {
  tasks
    .iter()
    .filter(|task| phase_of(task, phases) == Some(TaskPhase::Queued))
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
    MissingDetail::Limits => "tasks.readiness.missingLimits",
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
}

impl lurq::app::component::DevtoolsInspectable for TaskRow {}

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
    let detail = match (latest, task.status()) {
      (Some(latest), _) => latest.message(),
      (None, TaskStatus::Queued) => Message::new("tasks.progress.queued"),
      (None, TaskStatus::Draft) => Readiness::of(task).message(),
    };
    let latest = latest.and_then(Latest::run);
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
    }
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

/// Rows of `tab`, in task number order.
pub fn rows(tasks: &[Task], published: &[PipelineChoice], phases: &Phases, tab: TaskTab) -> Vec<TaskRow> {
  tasks
    .iter()
    .filter(|task| {
      tab.includes(task) && (tab != TaskTab::NeedsYou || phase_of(task, phases).is_some_and(TaskPhase::needs_you))
    })
    .map(|task| TaskRow::new(task, published, phases.get(&task.id())))
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
