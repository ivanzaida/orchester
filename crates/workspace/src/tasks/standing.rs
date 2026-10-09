//! What a task list row's dependency / readiness cell is about ([`ReadinessKind`]), chosen together with
//! its words, and the step a task's latest run is on ([`CurrentStep`]), for the row's run line.

use std::sync::Arc;

use orchester_core::{
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
  /// The step of `run`'s current attempt that is running, else the one that failed, else the last one
  /// it reached; `None` before its first attempt.
  pub fn of(run: &Run) -> Option<Self> {
    let executions = run.current_attempt()?.executions();
    let last_with = |status: StepStatus| executions.iter().rev().find(|execution| execution.status() == status);
    let execution = last_with(StepStatus::Running)
      .or_else(|| last_with(StepStatus::Failed))
      .or_else(|| executions.last())?;
    let key = execution.step();
    let snapshot = run.snapshot();
    let number = snapshot.steps().iter().position(|step| step.key() == key)? + 1;
    let name = snapshot
      .body()
      .step(key)
      .map_or_else(|| key.as_str().into(), |step| step.name.as_str().into());
    Some(Self { number, name })
  }
}
