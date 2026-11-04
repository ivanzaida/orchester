//! The coordinator's sink, as the executor sees it (`docs/architecture/remote-runners.md`, rule 2): everything the
//! executor records goes to it as a value that can be sent, and the coordinator writes it in today's transactions.
//! The run log goes this way: its events with the change they make to the run, attempts and step executions,
//! checkpoints, plans, raised step budgets, changed permissions and why the runner failed. So do the run's
//! invocations: each start, the transcript of their events and every change to their rows. A transcript's events are
//! redacted before they become a record. So do the task's conversation (agents' messages and how far people's got),
//! agents' questions and their tool requests, each change as the store applies it. So do steps' outputs and progress
//! lines, what reviews cost and the plan limits CLIs reported, the warm workspaces runs borrow and give back, the
//! removal of a run's worktrees, and base notices delivered or merged (ORC-681). The local sink is
//! `crate::execution::coordinator::sink`'s.

use orchester_core::integration::RemovalStart;
use orchester_store::{
  ActivityRecord, ApprovalRecord, InvocationRecord, InvocationStart, MessageRecord, NoticeRecord, QuestionRecord,
  RemovalRecord, RunLogRecord, RunStoreError, StepRecord, StoreError, WarmWorkspaceRecord, WorkspaceWritten,
};

pub(in crate::execution) trait Sink: Send {
  /// Writes `record` of a step's work (`orchester_store::ProjectDb::write_step`).
  fn step(&mut self, record: StepRecord) -> Result<(), StoreError>;

  /// Applies `record` to the project's warm workspaces, and gives what it did
  /// (`orchester_store::ProjectDb::write_workspace`).
  fn workspace(&mut self, record: WarmWorkspaceRecord) -> Result<WorkspaceWritten, StoreError>;

  /// Applies `record` to the record of a run's worktree removal: what beginning one found, for a record that begins
  /// one (`orchester_store::ProjectDb::write_removal`).
  fn removal(&mut self, record: RemovalRecord) -> Result<Option<RemovalStart>, StoreError>;

  /// Applies `record` to its run's pending base notice; whether it was still pending as read
  /// (`orchester_store::ProjectDb::write_notice`).
  fn notice(&mut self, record: NoticeRecord) -> Result<bool, StoreError>;
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

  /// Applies `record` to its task's conversation; whether it changed what it names
  /// (`orchester_store::ProjectDb::write_message`).
  fn message(&mut self, record: MessageRecord) -> Result<bool, StoreError>;

  /// Applies `record` to its run's questions; whether it changed the request it names
  /// (`orchester_store::ProjectDb::write_question`).
  fn question(&mut self, record: QuestionRecord) -> Result<bool, StoreError>;

  /// Applies `record` to its run's tool requests; whether it changed the requests it names
  /// (`orchester_store::ProjectDb::write_approval`).
  fn approval(&mut self, record: ApprovalRecord) -> Result<bool, StoreError>;

  /// Another sink of the same coordinator, for a task of the run's own: an agent's or a command's worker.
  fn another(&self) -> Box<dyn Sink>;
}
