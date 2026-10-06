//! Launches of agent CLIs and command tools' programs, and what they reported. A step execution has
//! one invocation per launch (the first, a retry, a continue); each keeps the CLI's session id, the
//! usage it reported and how it ended. The transcript is the invocation's [`AgentEvent`]s in order.
//! A command tool's run counts as an invocation too: its time is active time of the run.

use std::time::Duration;

use crate::{
  connection::stream::{AgentEvent, InvocationOutcome},
  id::{InvocationId, RunId, StepExecutionId},
  permission::LaunchPermissions,
  resource::ModelId,
  run::TokenUsage,
  time::Timestamp,
  tokens::TokensUsed,
};

/// What an invocation launched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationKind {
  /// An agent CLI, on `model`.
  Agent { model: ModelId },
  /// The program of a tool step's command tool.
  Command,
}

/// One launch of an agent CLI or a command tool's program, as recorded by the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
  pub id: InvocationId,
  pub run: RunId,
  pub execution: StepExecutionId,
  /// 1 for the execution's first launch.
  pub number: u32,
  pub kind: InvocationKind,
  /// The enforced policy used to prepare this agent launch, with what happened to commands outside
  /// it, recorded once before starting its process. `None` means unknown for older or unprepared
  /// agent launches, or no agent policy for command tools. Run permission changes never replace
  /// this snapshot.
  pub permissions: Option<LaunchPermissions>,
  /// The CLI's session id, once it reported one.
  pub session: Option<String>,
  pub started_at: Timestamp,
  /// `None` while the CLI runs.
  pub ended_at: Option<Timestamp>,
  /// Sum of the usage the CLI reported so far.
  pub usage: TokenUsage,
  pub outcome: Option<InvocationOutcome>,
  /// Time spent waiting for an interactive tool decision, excluded from the active limit.
  pub timing: InvocationTiming,
}

/// A persisted stop watch for one invocation's interactive approval waits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvocationTiming {
  inactive_ms: u64,
  waiting_since: Option<Timestamp>,
}

impl InvocationTiming {
  /// Restores the accounting saved before a runner restart.
  pub const fn restored(inactive_ms: u64, waiting_since: Option<Timestamp>) -> Self {
    Self {
      inactive_ms,
      waiting_since,
    }
  }

  pub const fn inactive_ms(&self) -> u64 {
    self.inactive_ms
  }

  pub const fn waiting_since(&self) -> Option<Timestamp> {
    self.waiting_since
  }

  /// Starts excluding time for a pending user decision. Repeated notification is idempotent.
  pub fn wait_for_person(&mut self, at: Timestamp) {
    if self.waiting_since.is_none() {
      self.waiting_since = Some(at);
    }
  }

  /// Stops excluding time after the person's decision has reached the live process.
  pub fn resume(&mut self, at: Timestamp) {
    if let Some(since) = self.waiting_since.take() {
      self.inactive_ms = self
        .inactive_ms
        .saturating_add(u64::try_from((at.unix_millis() - since.unix_millis()).max(0)).expect("nonnegative interval"));
    }
  }

  /// Active time of a launch at `started_at` that ended at `ended_at`, or still runs at `now`.
  pub fn active_millis(&self, started_at: Timestamp, ended_at: Option<Timestamp>, now: Timestamp) -> i64 {
    let end = ended_at.unwrap_or(now);
    (end.unix_millis() - started_at.unix_millis() - self.inactive_millis(end)).max(0)
  }

  fn inactive_millis(&self, end: Timestamp) -> i64 {
    let waiting = self
      .waiting_since
      .map_or(0, |since| (end.unix_millis() - since.unix_millis()).max(0));
    i64::try_from(self.inactive_ms)
      .unwrap_or(i64::MAX)
      .saturating_add(waiting)
  }
}

impl Invocation {
  /// The agent CLI's model; `None` for a command tool's program.
  pub const fn model(&self) -> Option<&ModelId> {
    match &self.kind {
      InvocationKind::Agent { model } => Some(model),
      InvocationKind::Command => None,
    }
  }

  /// Active agent time: from the launch to its end, or to `now` while it runs.
  pub fn active_millis(&self, now: Timestamp) -> i64 {
    self.timing.active_millis(self.started_at, self.ended_at, now)
  }
}

/// Active agent time of a run: the time its agent CLIs and command tools ran, up to `now` for one
/// that still runs. Time paused, stopped at a limit or waiting for a person has no invocation running, so it
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
