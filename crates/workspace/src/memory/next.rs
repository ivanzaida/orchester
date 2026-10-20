//! What the next agent of a task gets from memory (the Memory tab's *Next agent receives*): the agent step of the
//! task's latest run that works now or starts next, and the records core's recall gives that step's next
//! invocation ([`next_invocation_memory`]), the same records the runner would give it while nothing changes.

use std::sync::Arc;

use orchester_core::{
  memory::{Delivery, InjectionReason, Memory, MemoryLimits, MemoryStore, StepContext, next_invocation_memory},
  resource::AgentProfile,
  run::{Run, StepExecutor, StepStatus},
  store::{ResourceStore, RunStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// The step whose next invocation the panel shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextStep {
  /// Its position in the run's pipeline, from 1.
  pub number: usize,
  pub name: Arc<str>,
  /// The role of the agent profile version that runs it (*Builder*).
  pub agent: Arc<str>,
}

/// A record the next invocation gets, why, and its estimated tokens there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
  pub memory: Memory,
  pub reason: InjectionReason,
  pub tokens: u32,
  /// Cut to fit the token limit.
  pub truncated: bool,
}

/// What the next invocation of a task's agent step gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextAgent {
  pub step: NextStep,
  /// In prompt order: the latest handoff, pinned records, decisions and warnings, the best matches.
  pub given: Vec<Given>,
  /// Ranked records the record limit left out.
  pub over_records: usize,
  /// Ranked records whose tokens did not fit.
  pub over_tokens: usize,
  pub limits: MemoryLimits,
}

impl NextAgent {
  pub fn tokens(&self) -> u32 {
    self.given.iter().map(|given| given.tokens).sum()
  }
}

/// What the next agent of `task` gets, at `now`; `None` while no agent step of its latest run works or waits to
/// start: before its first run, after its latest run ended, or while that run cannot be read.
pub fn next_agent(db: &ProjectDb, task: &Task, now: Timestamp) -> Result<Option<NextAgent>, StoreError> {
  let Some(Ok(run)) = db.runs_of_task(task.id()).map_err(run_store_error)?.pop() else {
    return Ok(None);
  };
  if !run.status().has_live_work() {
    return Ok(None);
  }
  let Some((number, step, profile)) = next_step(&run) else {
    return Ok(None);
  };
  let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? else {
    return Ok(None);
  };
  let role = version.body().role().as_str();
  let worked = db.task_for_run(task.id(), run.id())?.unwrap_or_else(|| task.clone());
  let context = StepContext {
    task: &worked,
    step,
    profile: profile.resource,
    role,
  };
  let recalled = next_invocation_memory(db, context, now)?;
  let count = |delivery: Delivery| {
    recalled
      .left_out()
      .iter()
      .filter(|left| left.delivery == delivery)
      .count()
  };
  Ok(Some(NextAgent {
    step: NextStep {
      number,
      name: step.name.as_str().into(),
      agent: role.into(),
    },
    given: recalled
      .entries()
      .iter()
      .map(|entry| Given {
        memory: entry.memory.clone(),
        reason: entry.reason,
        tokens: entry.tokens,
        truncated: entry.truncated,
      })
      .collect(),
    over_records: count(Delivery::OverRecordLimit),
    over_tokens: count(Delivery::OverTokenLimit),
    limits: db.memory_settings()?.limits,
  }))
}

/// The agent step of `run`'s current attempt that works now, else the first that waits to start, in pipeline
/// order: its position, its step and the profile version that runs it.
fn next_step(
  run: &Run,
) -> Option<(
  usize,
  &orchester_core::resource::Step,
  &orchester_core::run::PinnedVersion,
)> {
  let attempt = run.current_attempt()?;
  let status = |step: &orchester_core::resource::Step| {
    attempt
      .executions()
      .iter()
      .find(|execution| *execution.step() == step.key)
      .map(orchester_core::run::StepExecution::status)
  };
  let agent_steps = || {
    run
      .snapshot()
      .body()
      .steps()
      .iter()
      .enumerate()
      .filter_map(|(index, step)| match run.snapshot().step(&step.key)?.executor() {
        StepExecutor::Agent { profile, .. } => Some((index + 1, step, profile)),
        StepExecutor::Tool { .. } => None,
      })
  };
  agent_steps()
    .find(|(_, step, _)| matches!(status(step), Some(StepStatus::Running | StepStatus::Waiting(_))))
    .or_else(|| agent_steps().find(|(_, step, _)| status(step) == Some(StepStatus::Pending)))
}
