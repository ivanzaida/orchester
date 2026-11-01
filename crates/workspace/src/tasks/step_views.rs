//! The steps of a run's view: each step of its pinned pipeline with its latest execution, executor, engine and
//! context use.

use std::num::NonZeroU32;

use orchester_core::{
  id::StepExecutionId,
  resource::StepKey,
  run::{Invocation, Run, StepExecutor},
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  detail::StepView,
  engines,
  step_executors::{Reached, executed},
};

/// The steps of `run`'s pinned pipeline in order, each with its latest execution, executor, engine and context use.
pub(super) fn step_views(db: &ProjectDb, run: &Run, invocations: &[Invocation]) -> Result<Vec<StepView>, StoreError> {
  let contexts = db.invocation_contexts(run.id())?;
  let mut steps = Vec::new();
  let stages = run.snapshot().body().stages();
  let stage_of = |key: &StepKey| {
    stages
      .iter()
      .position(|stage| stage.iter().any(|step| step.key == *key))
      .unwrap_or_default()
  };
  for (index, step) in run.snapshot().body().steps().iter().enumerate() {
    let executor = run.snapshot().step(&step.key).map(|pinned| pinned.executor());
    let (agent, reached, model, permissions) = executed(db, (run, invocations), &step.key, executor)?;
    let Reached {
      provider,
      endpoint,
      vendor,
    } = reached;
    let latest = run.latest_execution(&step.key);
    let attempt = latest.and_then(|execution| attempt_of(run, execution.id()));
    steps.push(StepView {
      number: index + 1,
      key: step.key.clone(),
      name: step.name.as_str().into(),
      agent,
      tool_step: matches!(executor, Some(StepExecutor::Tool { .. })),
      provider,
      endpoint,
      vendor,
      engine_recorded: false,
      effort: None,
      model,
      permissions,
      execution: latest.map(|execution| execution.id()),
      status: latest.map(|execution| execution.status()),
      attempt,
      failure: latest.and_then(|execution| execution.failure()),
      todo: latest
        .map(|execution| db.step_todo(run.id(), execution.id()))
        .transpose()?
        .flatten(),
      stage: stage_of(&step.key),
      context: super::context_uses::of_execution(&contexts, invocations, latest.map(|execution| execution.id())),
    });
  }
  engines::apply_recorded(db, &mut steps, invocations)?;
  Ok(steps)
}

/// The number of `run`'s attempt that holds `execution`.
fn attempt_of(run: &Run, execution: StepExecutionId) -> Option<NonZeroU32> {
  run
    .attempts()
    .iter()
    .find(|attempt| attempt.execution(execution).is_some())
    .map(orchester_core::run::Attempt::number)
}
