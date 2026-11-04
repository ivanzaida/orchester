//! What became of an action (`docs/architecture/team-authority-plan.md` §4, "Results and failure recovery"): a local
//! action's result, or one of a team command's explicit outcomes. A lost response is never read as a refusal, and a
//! committed command stays committed whatever happens on this computer afterwards: nothing undoes it.

use orchester_api::{
  client::AuthorityCallError,
  v2::{AuthorityError, CommandResult, LocalEffect, ModuleRead, TaskRead, TeamCommandValue, WorkspacePolicy},
};
use orchester_core::{
  id::OperationId,
  module::Module,
  task::{TaskUpdate, organization::Tag},
};
use orchester_store::StoreError;

#[derive(Debug)]
pub enum ActionOutcome {
  Local(LocalResult),
  Team(TeamOutcome),
}

/// A local action's result, as the project's writer gives it.
#[derive(Debug)]
pub enum LocalResult {
  /// A new task, or an edit of one: saved, or refused for a change made meanwhile with the task as it is now.
  Task(Box<TaskUpdate>),
  /// Whether an organization change or a move changed the task.
  Changed(bool),
  Module(Module),
  Tag(Tag),
  /// A tag renamed, merged or deleted.
  Tags,
}

/// A committed command's receipt: its value, with the local effects the server allowed, and the stream cursor just
/// after its events.
pub type Receipt = CommandResult<TeamCommandValue>;

#[derive(Debug)]
pub enum TeamOutcome {
  /// The server refused it. A `retryable` refusal (signed out, rate limited, unavailable, an unsupported client) left
  /// nothing committed and keeps the operation open, to send again under the same id; any other is final and kept.
  /// A revision conflict comes with the subject as the server holds it now, when the action has one to read.
  Rejected {
    operation: OperationId,
    error: AuthorityError,
    retryable: bool,
    current: Option<Current>,
  },
  /// Committed; `pending` are local effects of the receipt this computer has still to carry out (starting, stopping
  /// or delivering work: P4). The operation stays open until they are.
  AcceptedPending {
    receipt: Box<Receipt>,
    pending: Vec<LocalEffect>,
  },
  /// Committed, and every local effect of the receipt happened.
  Completed { receipt: Box<Receipt> },
  /// Committed, and a local effect of the receipt failed; the command stays committed, and the effect is made again
  /// when the operation is recovered.
  LocalEffectFailed { receipt: Box<Receipt>, error: StoreError },
  /// The response was lost or unreadable: the server may have committed it or not. The operation stays open; recover
  /// it, or send the same intent again, which looks it up before sending it again under the same id.
  AcceptanceUnknown { operation: OperationId },
}

/// The subject of a refused change as the server holds it now.
#[derive(Debug)]
pub enum Current {
  Task(Box<TaskRead>),
  Module(Box<ModuleRead>),
  Policy(WorkspacePolicy),
  /// It could not be read; the refusal stands without it.
  NotRead(AuthorityCallError),
}
