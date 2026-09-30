//! Task memory around an agent step: the records its invocation's prompt gets, recalled as it
//! starts with the project's Memory setting of that moment, and the handoff it leaves once its fate
//! is known ([`super::super::handoff`]).

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  memory::{MemoryStore, Recalled, StepContext, StepFate, next_invocation_memory},
  resource::Step,
  run::ProfileInfo,
  time::Timestamp,
};

use super::{super::run::RunTask, command::problem};
use crate::execution::{handoff::Keeper, run::ExecutionError};

impl RunTask {
  /// The records the next invocation of `step`, run by `profile`, gets
  /// ([`next_invocation_memory`]).
  pub(super) fn recall_memory(&self, step: &Step, profile: ProfileInfo<'_>) -> Result<Recalled, RunnerProblem> {
    let context = StepContext {
      task: &self.task,
      step,
      profile: profile.id,
      role: profile.body.role().as_str(),
    };
    next_invocation_memory(&self.db, context, Timestamp::now())
      .map_err(|error| problem(format!("cannot read task memory: {error}")))
  }

  /// How many memory records cannot be read back, while that was not reported in this run's
  /// transcript yet; agents get the others.
  pub(super) fn unreadable_memory(&self) -> Result<u64, RunnerProblem> {
    if self.unreadable_memory_reported {
      return Ok(0);
    }
    let unreadable = self
      .db
      .unreadable_memories()
      .map_err(|error| problem(format!("cannot read task memory: {error}")))?;
    for record in &unreadable {
      tracing::warn!(run = %self.run.id(), memory = %record.id, detail = %record.detail, "a memory record cannot be read");
    }
    Ok(u64::try_from(unreadable.len()).unwrap_or(u64::MAX))
  }

  /// Keeps the handoff of `execution`, which is about to pass or fail as `fate` says: before the
  /// transition is recorded and announced, so whoever sees the step end finds its handoff. A step
  /// waiting for its plan's approval keeps it when the plan is approved.
  pub(in super::super) fn keep_handoff(
    &mut self,
    execution: StepExecutionId,
    fate: StepFate,
  ) -> Result<(), ExecutionError> {
    let mut keeper = Keeper {
      db: &mut self.db,
      notifier: &self.notifier,
      vault: self.vault.as_ref(),
    };
    keeper.keep(&self.run, execution, fate)?;
    Ok(())
  }
}
