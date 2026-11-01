//! How full a step's agent session is (ORC-304), as its latest execution's invocations recorded it.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  connection::context_use::ContextUse,
  id::{InvocationId, RunId, StepExecutionId},
  run::Invocation,
  store::ActivityStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::detail::{RunView, StepView};
use crate::messages::{self, Message};

impl RunView {
  /// The agent steps whose session is at work now, in pipeline order: not a tool step, and its latest execution runs
  /// or waits for the person in its session. The context meter shows the first's reading (ORC-308).
  pub fn sessions_at_work(&self) -> impl Iterator<Item = &StepView> {
    self
      .working_steps()
      .filter(|step| !step.tool_step && step.execution.is_some())
  }

  /// The step whose session is at work, while `execution` is that step's latest execution; `None` for an earlier
  /// execution of it, a finished step's or a tool step's.
  pub fn session_at_work(&self, execution: StepExecutionId) -> Option<&StepView> {
    self.sessions_at_work().find(|step| step.execution == Some(execution))
  }
}

/// The latest reading of `execution`'s session: that of its latest invocation that recorded one, as a resumed
/// session's later invocation that reported nothing yet goes on with the context before. `None` while none did.
pub(super) fn of_execution(
  contexts: &HashMap<InvocationId, ContextUse>,
  invocations: &[Invocation],
  execution: Option<StepExecutionId>,
) -> Option<ContextUse> {
  let execution = execution?;
  invocations
    .iter()
    .rev()
    .filter(|invocation| invocation.execution == execution)
    .find_map(|invocation| contexts.get(&invocation.id).copied())
}

/// The latest reading of `execution` of `run` in the project at `root`, read alone: the context meter of the step
/// at work reads it again as its agent's turns arrive, without the run's detail or its transcript. Blocks on SQLite.
pub fn of_execution_in(root: &Path, run: RunId, execution: StepExecutionId) -> Result<Option<ContextUse>, Message> {
  let read = || -> Result<Option<ContextUse>, StoreError> {
    let db = ProjectDb::open(root)?;
    let contexts = db.invocation_contexts(run)?;
    let invocations = db.invocations(run)?;
    Ok(of_execution(&contexts, &invocations, Some(execution)))
  };
  read().map_err(|error| messages::store_error(&error))
}
