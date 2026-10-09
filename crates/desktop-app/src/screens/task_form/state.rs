//! What the form holds and does, read from its fields and its actions once per render: which task it is
//! about, whether it is saving or starting a run, what is wrong with the values, whether they differ
//! from what is stored, and what each action still needs (the reasons under the action bar, `s7jKkA`,
//! `i4Y9hL`, `Qp1B0`).

use std::sync::Arc;

use orchester_core::{
  task::{MissingDetail, Task, TaskDetails, TaskStatus},
  time::Timestamp,
};
use orchester_workspace::{messages::Message, tasks::data::PipelineChoice};

use super::{
  actions::QueuedTask,
  conflict::{Choices, Conflict},
  values::Values,
};

/// The task the form is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
  /// New task: nothing is stored until Create or Create & run.
  New,
  /// Edit task on a draft, whose details are stored as `task` holds them.
  Draft(Arc<Task>),
  /// Edit task on a task that has a run: changes apply to future runs, and it has no Create & run.
  Ran(Arc<Task>),
}

impl Subject {
  pub fn task(&self) -> Option<&Arc<Task>> {
    match self {
      Self::New => None,
      Self::Draft(task) | Self::Ran(task) => Some(task),
    }
  }

  pub const fn is_new(&self) -> bool {
    matches!(self, Self::New)
  }

  /// A task queued without a run, opened again: its first run is still to start.
  pub fn awaits_first_run(&self) -> bool {
    matches!(self, Self::Draft(task) if task.status() == TaskStatus::Queued)
  }
}

/// What the form is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
  Editing,
  /// Create or Save changes is under way.
  Saving,
  /// Create & run is saving and queuing the task.
  Creating,
  /// The task is queued and the runner is asked to start its run.
  Starting(QueuedTask),
  /// The task is queued and its run did not start, for the reasons the runner gave (none when the form
  /// was opened on the queued task).
  NotStarted {
    queued: QueuedTask,
    problems: Arc<[Message]>,
  },
}

/// What one of the actions still needs, said after "unavailable:".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
  Title,
  Brief,
  Criteria,
  Pipeline,
  NoPipelines,
  Limits,
  Budget,
  TimeLimit,
  /// A task that ran keeps every detail a run needs.
  KeepComplete,
  /// A newer version was saved under the edits: they are saved onto it first.
  NewerVersion,
}

impl Need {
  pub const fn key(self) -> &'static str {
    match self {
      Self::Title => "task_form.reasons.need.title",
      Self::Brief => "task_form.reasons.need.brief",
      Self::Criteria => "task_form.reasons.need.criteria",
      Self::Pipeline => "task_form.reasons.need.pipeline",
      Self::NoPipelines => "task_form.reasons.need.noPipelines",
      Self::Limits => "task_form.reasons.need.limits",
      Self::Budget => "task_form.reasons.need.budget",
      Self::TimeLimit => "task_form.reasons.need.timeLimit",
      Self::KeepComplete => "task_form.reasons.need.keepComplete",
      Self::NewerVersion => "task_form.reasons.need.newerVersion",
    }
  }
}

/// The form as one render reads it.
pub struct View {
  pub subject: Subject,
  pub phase: Phase,
  pub values: Values,
  /// Why the title, the budget or the time limit is not accepted.
  pub title_error: Option<Message>,
  pub budget_error: Option<Message>,
  pub time_error: Option<Message>,
  /// The details a save stores, when the values are valid.
  pub ready: Option<TaskDetails>,
  /// What a run still needs that the values do not have.
  pub missing: Vec<MissingDetail>,
  /// The values differ from what is stored (New task: from an empty form).
  pub changed: bool,
  /// The published pipelines; `None` while the project's tasks are read.
  pub pipelines: Option<Arc<Vec<PipelineChoice>>>,
  /// The changes were kept on this computer from an earlier visit, at this time.
  pub kept_at: Option<Timestamp>,
  /// Why the last save, or the last Create & run before queuing, failed.
  pub failure: Option<Arc<[Message]>>,
  /// The run being started was asked for by Retry run.
  pub retrying: bool,
  /// A newer version was saved while the fields hold edits.
  pub newer: bool,
  /// The newer version's fields to choose from, while the person chooses.
  pub conflict: Option<Conflict>,
  pub choices: Choices,
}

impl View {
  /// Fields cannot change while a save or a run request is under way, or while the task is queued.
  pub const fn locked(&self) -> bool {
    !matches!(self.phase, Phase::Editing)
  }

  /// The task is queued and its run has not started: its details stay as they are.
  pub const fn queued(&self) -> bool {
    matches!(self.phase, Phase::Starting(_) | Phase::NotStarted { .. })
  }

  /// The published pipeline the values name.
  pub fn pipeline(&self) -> Option<&PipelineChoice> {
    let id = self.values.pipeline?;
    self.pipelines.as_ref()?.iter().find(|choice| choice.id == id)
  }

  /// The project has no published pipeline to choose.
  pub fn no_pipelines(&self) -> bool {
    self.pipelines.as_ref().is_some_and(|pipelines| pipelines.is_empty())
  }

  /// What a save needs: a valid title and limits that are valid or left empty.
  fn save_needs(&self) -> Vec<Need> {
    let mut needs = Vec::new();
    if self.title_error.is_some() {
      needs.push(Need::Title);
    }
    if self.budget_error.is_some() {
      needs.push(Need::Budget);
    }
    if self.time_error.is_some() {
      needs.push(Need::TimeLimit);
    }
    needs
  }

  /// What Create still needs.
  pub fn create_needs(&self) -> Vec<Need> {
    self.save_needs()
  }

  /// What Save changes still needs; `None` while nothing differs from what is stored. A task that ran
  /// keeps every detail a run depends on (core's `TaskStatus::check_edit`).
  pub fn save_changes_needs(&self) -> Option<Vec<Need>> {
    if !self.changed {
      return None;
    }
    let mut needs = self.save_needs();
    let refused = match &self.subject {
      Subject::Ran(task) => task.status().check_edit(true, self.missing.clone()).is_err(),
      Subject::New | Subject::Draft(_) => false,
    };
    if needs.is_empty() && refused {
      needs.push(Need::KeepComplete);
    }
    Some(needs)
  }

  /// What Create & run still needs: whatever a save needs and every detail a run depends on.
  pub fn run_needs(&self) -> Vec<Need> {
    let mut needs = self.save_needs();
    if self.newer {
      needs.push(Need::NewerVersion);
    }
    for detail in &self.missing {
      needs.push(match detail {
        MissingDetail::Brief => Need::Brief,
        MissingDetail::AcceptanceCriteria => Need::Criteria,
        MissingDetail::Pipeline if self.no_pipelines() => Need::NoPipelines,
        MissingDetail::Pipeline => Need::Pipeline,
        MissingDetail::Limits => Need::Limits,
      });
    }
    // A pipeline that is no longer published cannot run, though the task names it.
    if self.values.pipeline.is_some() && self.pipelines.is_some() && self.pipeline().is_none() {
      needs.push(Need::Pipeline);
    }
    needs
  }
}
