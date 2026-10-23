//! The Integrate step: when syncing a run with its base branch conflicts, the run's work goes back, with the
//! conflicts, to the agent step the Deliver step's integrator profile works in, as *Ask an agent to resolve* sends it
//! (`tasks::return_work`). That agent merges the base and resolves them; the steps after it (Check, Review) run again,
//! and once the run passed every step again its delivery goes on. A pipeline whose integrator works in no step that
//! can write in the worktree has no Integrate step: its conflicts go to a person.

use orchester_core::{
  merge_queue::QueueState,
  resource::{DeliverStep, StepKey},
  revision::Revision,
  run::{Answerer, MergeConflicts, Run},
};
use orchester_ipc::{Command, WorkReturn};

use crate::{pipelines::deliver::integrating_step, tasks::return_work::conflict_note};

/// The step `run`'s conflicts go back to, among the agent steps it ran (`pipelines::deliver::integrating_step`).
pub(super) fn integrate_step(run: &Run, deliver: &DeliverStep) -> Option<StepKey> {
  let targets = run.ended_return_targets();
  integrating_step(run.snapshot().body(), deliver, |key| targets.contains(key)).map(|step| step.key.clone())
}

/// What sends `run`'s conflicts in `files`, with its base branch at `base`, to its Integrate step `step`, and the
/// state the run is in while it works on them; `None` for a run that names no base branch.
pub(super) fn integrate(
  run: &Run,
  step: StepKey,
  (base, files): (&Revision, Vec<String>),
) -> Option<(Command, QueueState)> {
  let branch = run.base_branch()?;
  let command = Command::ReturnRun(WorkReturn {
    run: run.id(),
    step: step.clone(),
    note: conflict_note(branch),
    conflicts: Some(MergeConflicts {
      base: branch.clone(),
      files: files.clone(),
    }),
    by: Answerer::Automation,
    expected: run.last_sequence(),
  });
  let state = QueueState::Integrating {
    base: base.clone(),
    step,
    files,
    after: run.last_sequence().get(),
  };
  Some((command, state))
}
