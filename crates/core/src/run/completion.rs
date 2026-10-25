//! A run completed without a merge (ORC-93): a run that passed every step and committed nothing, whose result is its
//! report or work done outside the repository, or one completed anyway although its pipeline expected changes.
//! Completing it sets its task Done, as a merge does, and keeps the last step's report as the run's artifact
//! *Report* ([`REPORT_ARTIFACT`]).

use std::{fmt, str::FromStr};

use thiserror::Error;

use super::{Answerer, Plan, Run, StepOutput};
use crate::{
  id::RunId,
  resource::{Deliverable, StepKey},
  task::Task,
  time::Timestamp,
};

/// What `run` of `task` delivers: what the task says, else what the pipeline version the run pinned says.
pub fn run_delivers(task: &Task, run: &Run) -> Deliverable {
  Deliverable::for_task(task.organization().delivers(), run.snapshot().body().delivers())
}

/// The name of the artifact a completed run keeps its last step's report as.
pub const REPORT_ARTIFACT: &str = "Report";

/// Who completed a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompletedBy {
  /// A person, from the task's Summary.
  Person,
  /// An MCP client: through Orchester's MCP server (`complete_run`), or with window input on the task's Summary.
  Automation,
  /// The Orchestrator, on its own, as it would merge the run (*Let the Orchestrator merge finished runs on its own*).
  Orchestrator,
}

impl CompletedBy {
  pub const ALL: [Self; 3] = [Self::Person, Self::Automation, Self::Orchestrator];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Person => "person",
      Self::Automation => "automation",
      Self::Orchestrator => "orchestrator",
    }
  }
}

impl From<Answerer> for CompletedBy {
  fn from(answerer: Answerer) -> Self {
    match answerer {
      Answerer::Person => Self::Person,
      Answerer::Automation => Self::Automation,
    }
  }
}

impl fmt::Display for CompletedBy {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(self.as_str())
  }
}

/// A stored completer that names none of [`CompletedBy::ALL`]; the database was edited or damaged.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not who completes a run")]
pub struct UnknownCompleter(String);

impl FromStr for CompletedBy {
  type Err = UnknownCompleter;

  fn from_str(value: &str) -> Result<Self, UnknownCompleter> {
    Self::ALL
      .into_iter()
      .find(|by| by.as_str() == value)
      .ok_or_else(|| UnknownCompleter(value.to_owned()))
  }
}

/// A run completed without a merge: by whom and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunCompletion {
  pub run: RunId,
  pub by: CompletedBy,
  pub completed_at: Timestamp,
}

/// A text a run keeps beside its branch, by name: today only a completed run's [`REPORT_ARTIFACT`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunArtifact {
  pub run: RunId,
  pub name: String,
  /// The step whose result it is.
  pub step: Option<StepKey>,
  pub text: String,
  pub created_at: Timestamp,
}

/// The report of `run`: what the last step of its pipeline that reported something left, as its latest execution's
/// output (a gated step's latest plan), with the step; `None` when no step reported anything.
pub fn last_report<'a>(run: &'a Run, outputs: &'a [StepOutput], plans: &'a [Plan]) -> Option<(&'a StepKey, &'a str)> {
  run.snapshot().body().steps().iter().rev().find_map(|step| {
    let execution = run.latest_execution(&step.key)?;
    let text = Plan::output_of(execution.id(), outputs, plans)?.trim();
    (!text.is_empty()).then_some((&step.key, text))
  })
}
