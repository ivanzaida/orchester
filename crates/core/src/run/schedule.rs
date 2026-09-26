//! Which step executions of a run may start now (`docs/design/orchester-workspace.md`,
//! "Pipelines": steps with the same satisfied dependencies run in parallel). A pending execution
//! of the current attempt is ready once every step it depends on has passed; how many of the
//! ready ones start is bounded by the agent slots the runner has free.

use super::{
  attempt::StepExecution,
  state::Run,
  status::{RunStatus, StepStatus},
};
use crate::id::StepExecutionId;

impl Run {
  /// Pending executions of the current attempt whose dependencies have all passed, in pipeline
  /// order.
  pub fn ready_executions(&self) -> Vec<StepExecutionId> {
    let Some(attempt) = self.current_attempt() else {
      return Vec::new();
    };
    let body = self.snapshot().body();
    attempt
      .executions()
      .iter()
      .filter(|execution| execution.status() == StepStatus::Pending)
      .filter(|execution| {
        body.step(execution.step()).is_some_and(|step| {
          step
            .depends_on
            .iter()
            .all(|dependency| self.latest_execution(dependency).map(StepExecution::status) == Some(StepStatus::Passed))
        })
      })
      .map(StepExecution::id)
      .collect()
  }

  /// Executions of the current attempt an agent works on (or worked on, when the run stopped
  /// mid-step): they continue before anything new starts.
  pub fn running_executions(&self) -> Vec<StepExecutionId> {
    self.current_attempt().map_or_else(Vec::new, |attempt| {
      attempt
        .executions()
        .iter()
        .filter(|execution| execution.status() == StepStatus::Running)
        .map(StepExecution::id)
        .collect()
    })
  }

  /// The ready executions to start with `free` agent slots: none while the run is not running or
  /// waits to pause, else the first `free` in pipeline order.
  pub fn executions_to_start(&self, free: usize) -> Vec<StepExecutionId> {
    if self.status() != &RunStatus::Running || self.pause_requested() {
      return Vec::new();
    }
    let mut ready = self.ready_executions();
    ready.truncate(free);
    ready
  }
}
