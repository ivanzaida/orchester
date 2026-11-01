//! How full a step's agent session is (ORC-304), as its latest execution's invocations recorded it.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  connection::context_use::ContextUse,
  id::{InvocationId, RunId, StepExecutionId},
  run::Invocation,
  store::ActivityStore,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

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
