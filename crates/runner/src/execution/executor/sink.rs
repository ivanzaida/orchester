//! The coordinator's sink, as the executor sees it (`docs/architecture/remote-runners.md`, rule 2): everything the
//! executor records goes to it as a value that can be sent, and the coordinator writes it in today's transactions.
//! The run log goes this way: its events with the change they make to the run, attempts and step executions,
//! checkpoints, plans, raised step budgets, changed permissions and why the runner failed. The local sink is
//! `crate::execution::coordinator::sink`'s.

use orchester_store::{RunLogRecord, RunStoreError};

pub(in crate::execution) trait Sink: Send {
  /// Writes `record` to the run's log, in one transaction, as `orchester_store::ProjectDb::write_run_log` does; a
  /// sequence conflict or a refused continuation comes back unchanged, as the store reported it.
  fn run_log(&mut self, record: RunLogRecord) -> Result<(), RunStoreError>;
}
