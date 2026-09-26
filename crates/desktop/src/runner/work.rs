//! Work the runners report ([`orchester_ipc::WorkSummary`]) as the desktop shows it: the rows of
//! the close confirmation (02.C `Xz30C`, 02.D `a0tTV`) and the status bar's Agents count.

use std::sync::Arc;

use orchester_core::{connection::AgentProvider, id::RunId};
use orchester_ipc::{RunWork, WorkState};

/// A run with an agent at work that closing the application would interrupt, as listed by the
/// close confirmation (`clVjO` rows in `um9B1`). Every value is runtime data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveWork {
  project: Arc<str>,
  run: RunId,
  task_key: Arc<str>,
  task_title: Arc<str>,
  step: Arc<str>,
  agent: Arc<str>,
  provider: AgentProvider,
  model: Arc<str>,
}

/// The fields of [`ActiveWork`], named so they cannot be swapped by position.
pub struct ActiveWorkFields<'a> {
  pub project: &'a str,
  pub run: RunId,
  pub task_key: &'a str,
  pub task_title: &'a str,
  pub step: &'a str,
  pub agent: &'a str,
  pub provider: AgentProvider,
  pub model: &'a str,
}

impl ActiveWork {
  pub fn new(fields: &ActiveWorkFields<'_>) -> Self {
    Self {
      project: fields.project.into(),
      run: fields.run,
      task_key: fields.task_key.into(),
      task_title: fields.task_title.into(),
      step: fields.step.into(),
      agent: fields.agent.into(),
      provider: fields.provider,
      model: fields.model.into(),
    }
  }

  /// `work` of a run in project `project`, as its runner reported it.
  pub(super) fn reported(project: &str, work: &RunWork) -> Self {
    Self::new(&ActiveWorkFields {
      project,
      run: work.run,
      task_key: &work.task_key,
      task_title: &work.task_title,
      step: &work.step,
      agent: &work.agent,
      provider: work.provider,
      model: &work.model,
    })
  }

  pub fn project(&self) -> &str {
    &self.project
  }

  pub fn run(&self) -> RunId {
    self.run
  }

  pub fn task_key(&self) -> &str {
    &self.task_key
  }

  pub fn task_title(&self) -> &str {
    &self.task_title
  }

  pub fn step(&self) -> &str {
    &self.step
  }

  pub fn agent(&self) -> &str {
    &self.agent
  }

  pub fn provider(&self) -> AgentProvider {
    self.provider
  }

  pub fn model(&self) -> &str {
    &self.model
  }
}

impl lurq::app::component::DevtoolsInspectable for ActiveWork {}

/// Where a run stands while the application closes after "Wait for agents to save" (02.D).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveState {
  /// Its agent finishes up to its next checkpoint.
  Saving,
  /// Settled: paused at this checkpoint (a full revision), or without a new one.
  Saved { checkpoint: Option<Arc<str>> },
}

impl SaveState {
  pub(super) fn of(state: &WorkState) -> Self {
    match state {
      WorkState::Working | WorkState::Saving => Self::Saving,
      WorkState::Saved { checkpoint } => Self::Saved {
        checkpoint: checkpoint.as_deref().map(Arc::from),
      },
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for SaveState {}
