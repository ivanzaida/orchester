//! What an agent's invocation records as it launches, sent to the coordinator's sink before its agent starts: the
//! policy it was prepared with, what it reaches its model through and on, what it was told, what it got of the code
//! index and of task memory, its first edit not made yet, and what it works without.

use orchester_core::{
  connection::stream::AgentEvent,
  run::{EditProgress, Invocation},
  time::Timestamp,
};
use orchester_store::InvocationRecord;

use super::{
  super::run::{ExecutionError, RunTask},
  start::AgentStart,
};

impl RunTask {
  /// Records what `invocation` launches with, from `start`.
  pub(super) fn record_launch(&mut self, invocation: &Invocation, start: &AgentStart) -> Result<(), ExecutionError> {
    let id = invocation.id;
    let mut records = vec![
      InvocationRecord::permissions(id, &start.permissions),
      InvocationRecord::connection(id, &start.connection),
    ];
    records.extend(start.engine.as_ref().map(|engine| InvocationRecord::engine(id, engine)));
    records.extend(start.account.map(|account| InvocationRecord::account(id, account)));
    records.extend(
      start
        .provider
        .as_ref()
        .map(|(base_url, key)| InvocationRecord::provider(id, base_url, *key)),
    );
    records.extend([
      InvocationRecord::prompt(id, &start.prompt),
      InvocationRecord::code_index(id, &start.code_index),
      InvocationRecord::edit_progress(id, &EditProgress::started()),
      InvocationRecord::memories(id, &start.memory),
    ]);
    for record in records {
      self.sink.invocation(record)?;
    }
    Ok(())
  }

  /// Records what the agent of `invocation` works without: the MCP servers and the codebase index of `left_out`.
  pub(super) fn record_left_out(
    &mut self,
    invocation: &Invocation,
    left_out: Vec<AgentEvent>,
  ) -> Result<(), ExecutionError> {
    if left_out.is_empty() {
      return Ok(());
    }
    let now = Timestamp::now();
    let events = left_out.into_iter().map(|event| (now, event)).collect();
    self.record_activity(invocation, events)
  }
}
