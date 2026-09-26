//! Launches of agent CLIs and what they reported. A step execution has one invocation per launch
//! (the first, a retry, a continue); each keeps the CLI's session id, the usage it reported and
//! how it ended. The transcript is the invocation's [`AgentEvent`]s in order.

use crate::{
  connection::stream::{AgentEvent, InvocationOutcome},
  id::{InvocationId, RunId, StepExecutionId},
  resource::ModelId,
  run::TokenUsage,
  time::Timestamp,
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
