//! The ready steps that wait for an agent slot, as the runner last reported them (ORC-403). Only a runner schedules,
//! and nothing stores what it said, so without one online no step waits for a slot.

use std::path::Path;

use orchester_core::{id::StepExecutionId, run::Run};

use crate::runner::{RunnerStatus, state::RunnerState};

/// The executions waiting for an agent slot in the project at `root`, by `status` of its runner.
pub fn slot_waits(status: Option<&RunnerStatus>, root: &Path) -> Vec<StepExecutionId> {
  status
    .filter(|status| status.project_root() == Some(root))
    .filter(|status| matches!(status.state(), RunnerState::Online | RunnerState::Stopping))
    .and_then(RunnerStatus::work)
    .map_or_else(Vec::new, |work| {
      work.slot_waits.iter().map(|wait| wait.execution).collect()
    })
}

/// The names of `run`'s steps among `waits`, in the order the runner reported them.
pub fn waiting_steps(run: &Run, waits: &[StepExecutionId]) -> Vec<String> {
  let body = run.snapshot().body();
  waits
    .iter()
    .filter_map(|wait| body.step(run.execution(*wait)?.step()))
    .map(|step| step.name.as_str().to_owned())
    .collect()
}
