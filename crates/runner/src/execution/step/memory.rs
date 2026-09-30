//! Task memory around an agent step: the records its invocation's prompt gets, recalled as it
//! starts with the project's Memory setting of that moment, and the handoff it leaves once its fate
//! is known ([`super::super::handoff`]).

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  memory::{MemoryStore, Recalled, StepContext, StepFate, next_invocation_memory},
  resource::Step,
  run::{Invocation, ProfileInfo},
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

  /// Reports under `invocation` how many memory records cannot be read back (agents get the
  /// others). The records are scanned once per run task, at its first invocation.
  pub(super) fn report_unreadable_memory(&mut self, invocation: &Invocation) -> Result<(), ExecutionError> {
    if self.memory_scanned {
      return Ok(());
    }
    self.memory_scanned = true;
    let unreadable = self.db.unreadable_memories()?;
    for record in &unreadable {
      tracing::warn!(run = %self.run.id(), memory = %record.id, detail = %record.detail, "a memory record cannot be read");
    }
    if unreadable.is_empty() {
      return Ok(());
    }
    let records = u64::try_from(unreadable.len()).expect("a count of records fits in 64 bits");
    self.report(invocation, RunnerProblem::MemoryUnreadable { records })
  }

  /// Keeps the handoff of `execution`, which is about to pass or fail as `fate` says: before the
  /// transition is recorded and announced, so whoever sees the step end finds its handoff. A step
  /// waiting for its plan's approval keeps it when the plan is approved.
  pub(in super::super) fn keep_handoff(&mut self, execution: StepExecutionId, fate: StepFate) {
    let mut keeper = Keeper {
      db: &mut self.db,
      notifier: &self.notifier,
      vault: self.vault.as_ref(),
    };
    keeper.keep_or_report(&self.run, execution, fate);
  }
}
