//! Launches of agent CLIs and what they reported. A step execution has one invocation per launch
//! (the first, a retry, a continue); each keeps the CLI's session id, the usage it reported and
//! how it ended. The transcript is the invocation's [`AgentEvent`]s in order.

use std::time::Duration;

use crate::{
  connection::stream::{AgentEvent, InvocationOutcome},
  id::{InvocationId, RunId, StepExecutionId},
  resource::ModelId,
  run::TokenUsage,
  time::Timestamp,
  tokens::TokensUsed,
};

/// One launch of an agent CLI, as recorded by the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
  pub id: InvocationId,
  pub run: RunId,
  pub execution: StepExecutionId,
  /// 1 for the execution's first launch.
  pub number: u32,
  pub model: ModelId,
  /// The CLI's session id, once it reported one.
  pub session: Option<String>,
  pub started_at: Timestamp,
  /// `None` while the CLI runs.
  pub ended_at: Option<Timestamp>,
  /// Sum of the usage the CLI reported so far.
  pub usage: TokenUsage,
  pub outcome: Option<InvocationOutcome>,
}

impl Invocation {
  /// Active agent time: from the launch to its end, or to `now` while it runs.
  pub fn active_millis(&self, now: Timestamp) -> i64 {
    let end = self.ended_at.unwrap_or(now);
    (end.unix_millis() - self.started_at.unix_millis()).max(0)
  }
}

/// Active agent time of a run: the time its agent CLIs ran, up to `now` for one that still
/// runs. Time paused, stopped at a limit or waiting for a person has no invocation running, so it
/// does not count (`docs/design/orchester-workspace.md`, "Stopped at limit").
pub fn active_time(invocations: &[Invocation], now: Timestamp) -> Duration {
  let millis: i64 = invocations.iter().map(|invocation| invocation.active_millis(now)).sum();
  Duration::from_millis(u64::try_from(millis).expect("active times are never negative"))
}

/// Tokens `execution` used over all its invocations, as far as they count toward its step's
/// budget.
pub fn execution_tokens(invocations: &[Invocation], execution: StepExecutionId) -> TokensUsed {
  invocations
    .iter()
    .filter(|invocation| invocation.execution == execution)
    .map(|invocation| invocation.usage.counted_tokens())
    .sum()
}

/// One stored transcript entry of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityEntry {
  /// Position in the run's transcript, from 1 without gaps.
  pub sequence: u64,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  pub at: Timestamp,
  pub event: AgentEvent,
}
