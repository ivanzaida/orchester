//! A step's header in the timeline (`mDnc3`): its number, step and agent, the engine and invocations
//! with their times, the tokens it used and its state; or, after a failure, a step of the attempt
//! that never ran.

use std::sync::Arc;

use orchester_core::{
  connection::{AgentProvider, EngineVendor},
  id::StepExecutionId,
  run::{EditProgress, RunStatus, StepStatus, TokenUsage},
  time::Timestamp,
};

use crate::tasks::detail::RunView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepHead {
  /// `None` for a step that never ran after a failure.
  pub execution: Option<StepExecutionId>,
  pub number: usize,
  pub step: Arc<str>,
  /// The agent role, or the tool's name for a tool step.
  pub agent: Arc<str>,
  /// A tool step: a command tool ran instead of an agent, on no model and with no tokens.
  pub command: bool,
  pub provider: Option<AgentProvider>,
  pub vendor: Option<EngineVendor>,
  pub model: Arc<str>,
  pub invocations: usize,
  pub started: Option<Timestamp>,
  /// `None` while an invocation runs.
  pub ended: Option<Timestamp>,
  pub usage: TokenUsage,
  /// What the execution used before its first edit; `None` for a tool step or launches recorded
  /// before it was kept.
  pub before_edit: Option<EditProgress>,
  pub status: Option<StepStatus>,
  /// The attempt it ran in, when the run had more than one.
  pub attempt: Option<u32>,
}

impl StepHead {
  /// The header of `execution`, which ran; `None` when its step is not in the run's pipeline.
  pub fn of(run: &RunView, execution: StepExecutionId) -> Option<Self> {
    let step = run.step_of(execution)?;
    let status = run.run.execution(execution)?.status();
    let invocations: Vec<_> = run.invocations_of(execution).collect();
    let running = invocations.iter().any(|invocation| invocation.ended_at.is_none());
    let attempt = (run.run.attempts().len() > 1)
      .then(|| {
        run
          .run
          .attempts()
          .iter()
          .find(|attempt| attempt.execution(execution).is_some())
          .map(|attempt| attempt.number().get())
      })
      .flatten();
    Some(Self {
      execution: Some(execution),
      number: step.number,
      step: step.name.clone(),
      agent: step.agent.clone().unwrap_or_else(|| "".into()),
      command: step.tool_step,
      provider: step.provider,
      vendor: step.vendor.clone(),
      model: step.model.clone().unwrap_or_else(|| "".into()),
      invocations: invocations.len(),
      started: invocations.first().map(|invocation| invocation.started_at),
      ended: if running {
        None
      } else {
        invocations.last().and_then(|invocation| invocation.ended_at)
      },
      usage: invocations.iter().map(|invocation| invocation.usage).sum(),
      before_edit: run.index.execution_edit(execution),
      status: Some(status),
      attempt,
    })
  }

  /// The steps of a failed run's last attempt that never ran, in pipeline order (`hep0l`).
  pub fn not_started(run: &RunView) -> Vec<Self> {
    if !matches!(run.run.status(), RunStatus::Failed(_)) {
      return Vec::new();
    }
    run
      .steps
      .iter()
      .filter(|step| {
        step.execution.is_none_or(|execution| {
          run.invocations_of(execution).next().is_none()
            && matches!(step.status, None | Some(StepStatus::Pending | StepStatus::Skipped))
        })
      })
      .map(|step| Self {
        execution: None,
        number: step.number,
        step: step.name.clone(),
        agent: step.agent.clone().unwrap_or_else(|| "".into()),
        command: step.tool_step,
        provider: step.provider,
        vendor: step.vendor.clone(),
        model: step.model.clone().unwrap_or_else(|| "".into()),
        invocations: 0,
        started: None,
        ended: None,
        usage: TokenUsage::default(),
        before_edit: None,
        status: None,
        attempt: None,
      })
      .collect()
  }

  /// Whether it starts open: running, waiting for the person, or failed, which shows why (`hep0l`).
  pub fn starts_open(&self) -> bool {
    (self.ended.is_none() && self.execution.is_some())
      || matches!(self.status, Some(StepStatus::Waiting(_) | StepStatus::Failed))
  }
}
