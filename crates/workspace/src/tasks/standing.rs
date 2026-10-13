//! What a task list row's dependency / readiness cell is about ([`ReadinessKind`]), chosen together with
//! its words, and the step a task's latest run is on ([`CurrentStep`]), for the row's run line, or that it
//! waits to be delivered ([`RunStage`]).

use std::sync::Arc;

use orchester_core::{
  id::StepExecutionId,
  run::{Run, StepStatus, TaskPhase},
  task_group::GroupHold,
};

/// What a row's readiness words are about, so a screen can draw them without reading the words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadinessKind {
  /// A draft with every detail: it can be queued.
  Ready,
  /// A draft that still misses details.
  Missing,
  /// Its reviewed group holds it, for this reason.
  Held(GroupHold),
  /// Another task blocks it, or it is marked Blocked.
  Blocked,
  /// Kept *Not shared* when the project joined a server project.
  NotShared,
  /// Where its run stands, or that it waits for its first run (`Queued`).
  Run(TaskPhase),
  /// Its latest run cannot be read.
  Unreadable,
}

/// The step a run's agent is on, or the one it stopped at: its 1-based place in the pipeline and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentStep {
  pub number: usize,
  pub name: Arc<str>,
}

impl CurrentStep {
  /// Where `run` stands, by the same rule as its task page (`detail::RunView::current_step`): by each
  /// step's latest execution in pipeline order, the first step that is running or waits for a person, else
  /// the first that failed, else the last that started. A step that has not started is never where the run
  /// stands; `None` before its first execution.
  pub fn of(run: &Run) -> Option<Self> {
    let steps: Vec<(usize, &str, StepStatus)> = run
      .snapshot()
      .body()
      .steps()
      .iter()
      .enumerate()
      .filter_map(|(index, step)| Some((index + 1, step.name.as_str(), run.latest_execution(&step.key)?.status())))
      .collect();
    let (number, name, _) = steps
      .iter()
      .find(|(.., status)| matches!(status, StepStatus::Running | StepStatus::Waiting(_)))
      .or_else(|| steps.iter().find(|(.., status)| *status == StepStatus::Failed))
      .or_else(|| steps.iter().rev().find(|(.., status)| *status != StepStatus::Pending))?;
    Some(Self {
      number: *number,
      name: (*name).into(),
    })
  }

  /// The step `execution` of `run` executes.
  pub fn of_execution(run: &Run, execution: StepExecutionId) -> Option<Self> {
    let key = run.execution(execution)?.step();
    let snapshot = run.snapshot();
    let number = snapshot.steps().iter().position(|step| step.key() == key)? + 1;
    let name = snapshot
      .body()
      .step(key)
      .map_or_else(|| key.as_str().into(), |step| step.name.as_str().into());
    Some(Self { number, name })
  }
}

/// Where a run stands in its pipeline, as the Overview's status names it (`XirGV`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStage<'a> {
  /// The step it is on, waits at or stopped at (`Needs input · 02 Plan`).
  Step(&'a CurrentStep),
  /// Its work passed every step and waits to be merged (`Ready to merge · Deliver`).
  Deliver,
}

impl<'a> RunStage<'a> {
  /// Where the latest run of a task that stands at `phase` is, at `step`.
  pub fn of(phase: Option<TaskPhase>, step: Option<&'a CurrentStep>) -> Option<Self> {
    if phase == Some(TaskPhase::Succeeded) {
      return Some(Self::Deliver);
    }
    step.map(Self::Step)
  }
}
