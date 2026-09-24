//! The task list (02.03 `J9467`) and the overview's task card (02.08 `Twtqc`): tabs, which
//! tasks each shows, and what a row says about a task.

use std::sync::Arc;

use orchester_core::{
  id::{ResourceId, TaskId},
  resource::VersionNumber,
  task::{MissingDetail, Task, TaskStatus},
  time::Timestamp,
};

use crate::{messages::Message, tasks::data::PipelineChoice};

/// Tabs of the task list. Only tabs whose tasks can be told apart today are available;
/// queueing, review and completion do not exist yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskTab {
  All,
  Drafts,
  Queue,
  NeedsReview,
  Done,
}

impl lurq::app::component::DevtoolsInspectable for TaskTab {}

impl TaskTab {
  pub const ALL: [Self; 5] = [Self::All, Self::Drafts, Self::Queue, Self::NeedsReview, Self::Done];

  pub const fn is_available(self) -> bool {
    matches!(self, Self::All | Self::Drafts)
  }

  pub const fn label_key(self) -> &'static str {
    match self {
      Self::All => "tasks.tabs.all",
      Self::Drafts => "tasks.tabs.drafts",
      Self::Queue => "tasks.tabs.queue",
      Self::NeedsReview => "tasks.tabs.needsReview",
      Self::Done => "tasks.tabs.done",
    }
  }

  /// Whether `task` belongs to this tab; unavailable tabs contain nothing.
  pub fn includes(self, task: &Task) -> bool {
    match self {
      Self::All => true,
      Self::Drafts => task.status() == TaskStatus::Draft,
      Self::Queue | Self::NeedsReview | Self::Done => false,
    }
  }

  /// Number of tasks in the tab, or `None` for a tab that cannot be counted yet.
  pub fn count(self, tasks: &[Task]) -> Option<usize> {
    self
      .is_available()
      .then(|| tasks.iter().filter(|task| self.includes(task)).count())
  }
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
  pub id: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub pipeline: PipelineLabel,
  pub status: TaskStatus,
  pub readiness: Readiness,
  pub created_at: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for TaskRow {}

impl TaskRow {
  pub fn new(task: &Task, published: &[PipelineChoice]) -> Self {
    Self {
      id: task.id(),
      key: task.key().to_string().into(),
      title: task.details().title.as_str().into(),
      pipeline: PipelineLabel::of(task.details().pipeline, published),
      status: task.status(),
      readiness: Readiness::of(task),
      created_at: task.created_at(),
    }
  }
}

/// Rows of `tab`, in task number order.
pub fn rows(tasks: &[Task], published: &[PipelineChoice], tab: TaskTab) -> Vec<TaskRow> {
  tasks
    .iter()
    .filter(|task| tab.includes(task))
    .map(|task| TaskRow::new(task, published))
    .collect()
}

/// The newest `limit` tasks, newest first, for the overview.
pub fn recent(tasks: &[Task], published: &[PipelineChoice], limit: usize) -> Vec<TaskRow> {
  tasks
    .iter()
    .rev()
    .take(limit)
    .map(|task| TaskRow::new(task, published))
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
