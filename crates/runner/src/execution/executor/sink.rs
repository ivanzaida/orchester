//! The coordinator's sink, as the executor sees it (`docs/architecture/remote-runners.md`, rule 2): everything the
//! executor records goes to it as a value that can be sent, and the coordinator writes it in today's transactions.
//! The run log goes this way: its events with the change they make to the run, attempts and step executions,
//! checkpoints, plans, raised step budgets, changed permissions and why the runner failed. So do the run's
//! invocations: each start, the transcript of their events and every change to their rows. A transcript's events are
//! redacted before they become a record. So do the task's conversation (agents' messages and how far people's got),
//! agents' questions and their tool requests, each change as the store applies it. The local sink is
//! `crate::execution::coordinator::sink`'s.
//!
//! Rows the desktop owns are written only by the coordinator (rule 3): the executor asks for them here, and the
//! answer tells it what it needs to go on. These are approval rules, Always allow, a connection's model requirement
//! and agents' memory records.

use orchester_store::{
  ActivityRecord, AlwaysAllowAnswer, AlwaysAllowRecord, ApprovalRecord, InvocationRecord, InvocationStart,
  MemoryAnswer, MemoryRecord, MessageRecord, ModelRequirement, QuestionRecord, RuleAnswer, RuleRecord, RunLogRecord,
  RunStoreError, StoreError,
};

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

  /// Applies `record` to its task's conversation; whether it changed what it names
  /// (`orchester_store::ProjectDb::write_message`).
  fn message(&mut self, record: MessageRecord) -> Result<bool, StoreError>;

  /// Applies `record` to its run's questions; whether it changed the request it names
  /// (`orchester_store::ProjectDb::write_question`).
  fn question(&mut self, record: QuestionRecord) -> Result<bool, StoreError>;

  /// Applies `record` to its run's tool requests; whether it changed the requests it names
  /// (`orchester_store::ProjectDb::write_approval`).
  fn approval(&mut self, record: ApprovalRecord) -> Result<bool, StoreError>;

  /// Adds an approval rule, or counts a request a project's rule answered (`orchester_store::ProjectDb::write_rule`).
  fn rule(&mut self, record: RuleRecord) -> Result<RuleAnswer, StoreError>;

  /// Leaves what Always allow promises, or says it is not available
  /// (`orchester_store::ProjectDb::write_always_allow`).
  fn always_allow(&mut self, record: AlwaysAllowRecord) -> Result<AlwaysAllowAnswer, StoreError>;

  /// Records the CLI version a connection's model needs (`orchester_store::ProjectDb::write_model_requirement`).
  fn model_requirement(&mut self, requirement: ModelRequirement) -> Result<(), StoreError>;

  /// Keeps an agent's memory write, its content already screened (`orchester_store::ProjectDb::write_memory`).
  fn keep_memory(&mut self, record: MemoryRecord) -> Result<MemoryAnswer, StoreError>;

  /// Another sink of the same coordinator, for a task of the run's own: an agent's or a command's worker.
  fn another(&self) -> Box<dyn Sink>;
}
