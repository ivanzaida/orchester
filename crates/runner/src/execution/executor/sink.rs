//! The coordinator's sink, as the executor sees it (`docs/architecture/remote-runners.md`, rule 2): everything the
//! executor records goes to it as a value that can be sent, and the coordinator writes it in today's transactions.
//! The run log goes this way: its events with the change they make to the run, attempts and step executions,
//! checkpoints, plans, raised step budgets, changed permissions and why the runner failed. So do the run's
//! invocations: each start, the transcript of their events and every change to their rows. A transcript's events are
//! redacted before they become a record. The local sink is `crate::execution::coordinator::sink`'s.

use orchester_store::{ActivityRecord, InvocationRecord, InvocationStart, RunLogRecord, RunStoreError, StoreError};

pub(in crate::execution) trait Sink: Send {
  /// Writes `record` to the run's log, in one transaction, as `orchester_store::ProjectDb::write_run_log` does; a
  /// sequence conflict or a refused continuation comes back unchanged, as the store reported it.
  fn run_log(&mut self, record: RunLogRecord) -> Result<(), RunStoreError>;

  /// Records `start` and gives its number among its execution's invocations
  /// (`orchester_store::ProjectDb::write_invocation_start`).
  fn invocation_start(&mut self, start: InvocationStart) -> Result<u32, StoreError>;

  /// Appends `record` to its run's transcript and gives the sequence the desktop is told of
  /// (`orchester_store::ProjectDb::write_activity`).
  fn activity(&mut self, record: ActivityRecord) -> Result<u64, StoreError>;

  /// Applies `record` to its invocation's rows (`orchester_store::ProjectDb::write_invocation`).
  fn invocation(&mut self, record: InvocationRecord) -> Result<(), StoreError>;

  /// Another sink of the same coordinator, for a task of the run's own: an agent's or a command's worker.
  fn another(&self) -> Box<dyn Sink>;
}
