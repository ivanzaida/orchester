//! How full a step's agent session is (ORC-304), as its latest execution's invocations recorded it.

use std::collections::HashMap;

use orchester_core::{
  connection::context_use::ContextUse,
  id::{InvocationId, StepExecutionId},
  run::Invocation,
};

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
