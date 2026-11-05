//! A team command's operation (S4): its id and request are recorded before the request is sent, so a response that is
//! lost leaves an open operation, never a guess. An open operation is first looked up on the server, then sent again
//! with the same id and the same bytes only when the server has no outcome of it, so a retry never commits twice. A
//! committed receipt is kept before its local effects are made, and a refusal is kept as it came; the same intent sent
//! again reads that back. The request's digest binds the id to the request: another request under the same id is
//! refused here, and by the server too. Two dispatches of one intent at once may both send it, which the server answers
//! with one receipt; whichever settles it first decides what is kept, and the other returns that.

use std::fmt::Write as _;

use orchester_api::{
  client::AuthorityCallError,
  v2::{AuthorityError, AuthorityErrorCode, CommandRequest, ErrorDetails, OperationRead, Subject, TeamCommand},
};
use orchester_core::{
  blob::ContentHash,
  id::{ModuleId, OperationId, TaskId},
  time::Timestamp,
};
use orchester_store::{NewTeamOperation, ProjectDb, TeamOperation, TeamOperationState, TeamSettlement};
use sha2::{Digest as _, Sha256};

use super::{
  Accepted, ActionError, Current, Receipt, Refusal, TeamOutcome, command::TeamRequest, effects, ownership::TeamLink,
};
use crate::team_reads::TeamSession;

/// One team project's operations in one session.
pub(super) struct Operations<'a> {
  pub(super) db: &'a mut ProjectDb,
  pub(super) session: &'a TeamSession,
  pub(super) link: TeamLink,
}

impl Operations<'_> {
  /// Records `request` as operation `operation` and sends it; the same intent recorded before is resumed instead.
  pub(super) async fn dispatch(
    &mut self,
    operation: OperationId,
    request: TeamRequest,
  ) -> Result<TeamOutcome, ActionError> {
    let body = CommandRequest {
      operation_id: operation,
      expected: request.expected,
      command: request.command,
    };
    let digest = digest(&body);
    if let Some(known) = self.db.team_operation(operation)? {
      if known.digest != digest {
        return Err(ActionError::OperationMismatch(operation));
      }
      return self.resume(known).await;
    }
    // The contract's types serialize to JSON without fail: they hold no map with non-string keys.
    let text = serde_json::to_string(&body).expect("a command request serializes");
    let new = NewTeamOperation {
      id: operation,
      kind: body.command.kind(),
      digest: &digest,
      request: &text,
    };
    let recorded = self
      .db
      .begin_team_operation(new, Timestamp::now())
      .map_err(ActionError::of_record)?;
    if recorded.state != TeamOperationState::Sent {
      return self.resume(recorded).await;
    }
    self.send(&body).await
  }

  /// Carries on with the recorded operation `record` from where it stands; one recorded for another server project
  /// than this link's is [`ActionError::OperationOfOtherProject`].
  pub(super) async fn resume(&mut self, record: TeamOperation) -> Result<TeamOutcome, ActionError> {
    if record.server != self.link.server || record.project != self.link.project {
      return Err(ActionError::OperationOfOtherProject(record.id));
    }
    if let Some(outcome) = kept(&record)? {
      return Ok(outcome);
    }
    if record.state == TeamOperationState::Sent {
      return self.look_up(&record).await;
    }
    let text = record.receipt.ok_or(ActionError::Unreadable(record.id))?;
    let receipt = read(record.id, &text)?;
    self.committed(receipt, &text)
  }

  /// What the server made of an operation sent before; sent again when the server has no outcome of it. A lookup the
  /// server refuses for good, e.g. for a caller who may no longer see the operation's outcome, is its refusal, and so
  /// is one refused as of a desktop older than it accepts.
  async fn look_up(&mut self, record: &TeamOperation) -> Result<TeamOutcome, ActionError> {
    self.ensure_session()?;
    let body: CommandRequest<TeamCommand> = read(record.id, &record.request)?;
    match self.session.authority().operation(record.id).await {
      Ok(OperationRead::Committed { response, .. }) => self.receipt(record.id, response),
      Ok(OperationRead::Rejected { error, .. }) => self.refused(record.id, &body, error).await,
      Err(AuthorityCallError::Refused { error, .. }) if error.code() == AuthorityErrorCode::NotFound => {
        self.send(&body).await
      }
      Err(AuthorityCallError::Refused { error, .. })
        if !retryable(error.code()) || error.code() == AuthorityErrorCode::UnsupportedContract =>
      {
        self.refused(record.id, &body, error).await
      }
      Err(_) => Ok(TeamOutcome::AcceptanceUnknown { operation: record.id }),
    }
  }

  async fn send(&mut self, body: &CommandRequest<TeamCommand>) -> Result<TeamOutcome, ActionError> {
    self.ensure_session()?;
    match self.session.authority().command(body).await {
      Ok(receipt) => self.receipt(body.operation_id, receipt),
      Err(AuthorityCallError::Refused { error, .. }) => self.refused(body.operation_id, body, error).await,
      Err(AuthorityCallError::Client(_)) => Ok(TeamOutcome::AcceptanceUnknown {
        operation: body.operation_id,
      }),
    }
  }

  fn ensure_session(&self) -> Result<(), ActionError> {
    if !self.session.current() {
      return Err(ActionError::SessionChanged);
    }
    Ok(())
  }

  fn receipt(&mut self, operation: OperationId, receipt: Receipt) -> Result<TeamOutcome, ActionError> {
    if receipt.operation_id != operation {
      return Err(ActionError::ReceiptMismatch(operation));
    }
    let text = serde_json::to_string(&receipt).expect("a receipt serializes");
    self.committed(receipt, &text)
  }

  /// Keeps the committed `receipt` first, then makes its local effects; it stays committed whatever they do.
  fn committed(&mut self, receipt: Receipt, text: &str) -> Result<TeamOutcome, ActionError> {
    let operation = receipt.operation_id;
    if let Some(kept) = self.settle(operation, TeamSettlement::Committed { receipt: text })? {
      return Ok(kept);
    }
    let pending = match effects::apply(self.db, self.link, &receipt.result) {
      Ok(pending) => pending,
      Err(error) => {
        if let Some(kept) = self.settle(operation, TeamSettlement::EffectFailed { receipt: text })? {
          return Ok(kept);
        }
        return Ok(TeamOutcome::LocalEffectFailed {
          receipt: Box::new(receipt),
          error,
        });
      }
    };
    if !pending.is_empty() {
      return Ok(TeamOutcome::AcceptedPending {
        receipt: Box::new(receipt),
        pending,
      });
    }
    if let Some(kept) = self.settle(operation, TeamSettlement::Completed { receipt: text })? {
      return Ok(kept);
    }
    Ok(TeamOutcome::Completed(Accepted::Command(Box::new(receipt))))
  }

  /// The server's refusal of `body`: kept unless retryable, with the subject as it is now after a revision conflict,
  /// and the version the server needs when it refused this desktop's.
  async fn refused(
    &mut self,
    operation: OperationId,
    body: &CommandRequest<TeamCommand>,
    error: AuthorityError,
  ) -> Result<TeamOutcome, ActionError> {
    let retryable = retryable(error.code());
    if !retryable {
      let text = serde_json::to_string(&error).expect("a refusal serializes");
      if let Some(kept) = self.settle(operation, TeamSettlement::Rejected { rejection: &text })? {
        return Ok(kept);
      }
    }
    let current = match (error.code(), readable(&body.command)) {
      (AuthorityErrorCode::RevisionConflict, Some(subject)) => Some(self.current(subject).await),
      _ => None,
    };
    let refusal = match error.details() {
      ErrorDetails::UnsupportedContract { required_version } => Refusal::NeedsVersion {
        required: required_version.clone(),
      },
      ErrorDetails::None | ErrorDetails::DecisionRequired { .. } => Refusal::Command(error),
    };
    Ok(TeamOutcome::Rejected {
      operation,
      refusal,
      retryable,
      current,
    })
  }

  /// Records `settlement` of `operation`; `None` once recorded. When another dispatch of the same intent settled it
  /// for good first, the database refuses another outcome and the one kept is returned instead.
  fn settle(
    &mut self,
    operation: OperationId,
    settlement: TeamSettlement<'_>,
  ) -> Result<Option<TeamOutcome>, ActionError> {
    let Err(error) = self.db.settle_team_operation(operation, settlement, Timestamp::now()) else {
      return Ok(None);
    };
    let Some(record) = self.db.team_operation(operation)? else {
      return Err(error.into());
    };
    match kept(&record)? {
      Some(outcome) => Ok(Some(outcome)),
      None => Err(error.into()),
    }
  }

  async fn current(&self, subject: Readable) -> Current {
    let authority = self.session.authority();
    let read = match subject {
      Readable::Task(task) => authority.task(task).await.map(|read| Current::TaskRead(Box::new(read))),
      Readable::Module(module) => authority
        .module(module)
        .await
        .map(|summary| Current::ModuleRead(Box::new(summary.module))),
    };
    read.unwrap_or_else(Current::NotRead)
  }
}

/// The outcome kept for an operation settled for good, completed or rejected; `None` while it is still open.
fn kept(record: &TeamOperation) -> Result<Option<TeamOutcome>, ActionError> {
  let operation = record.id;
  let outcome = match record.state {
    TeamOperationState::Completed => {
      let text = record.receipt.as_deref().ok_or(ActionError::Unreadable(operation))?;
      TeamOutcome::Completed(Accepted::Command(Box::new(read(operation, text)?)))
    }
    TeamOperationState::Rejected => {
      let text = record.rejection.as_deref().ok_or(ActionError::Unreadable(operation))?;
      TeamOutcome::Rejected {
        operation,
        refusal: Refusal::Command(read(operation, text)?),
        retryable: false,
        current: None,
      }
    }
    TeamOperationState::Sent | TeamOperationState::Committed | TeamOperationState::EffectFailed => return Ok(None),
  };
  Ok(Some(outcome))
}

/// A subject whose current value the server reads after a revision conflict.
#[derive(Clone, Copy, Debug)]
enum Readable {
  Task(TaskId),
  Module(ModuleId),
}

/// The revision-checked subject of `command`.
const fn readable(command: &TeamCommand) -> Option<Readable> {
  match command {
    TeamCommand::AssignmentSet { subject, .. } => Some(match subject {
      Subject::Task { id } => Readable::Task(*id),
      Subject::Module { id } => Readable::Module(*id),
    }),
    TeamCommand::ModuleBranchSet { module_id, .. } => Some(Readable::Module(*module_id)),
    TeamCommand::WorkStart { authority, .. } => match authority.subject {
      Subject::Task { id } => Some(Readable::Task(id)),
      Subject::Module { id } => Some(Readable::Module(id)),
    },
    TeamCommand::TaskCreate { .. }
    | TeamCommand::ClaimAcquire { .. }
    | TeamCommand::ClaimTakeOver { .. }
    | TeamCommand::ClaimRelease { .. }
    | TeamCommand::DeliveryAuthorize { .. }
    | TeamCommand::DeliveryRequest { .. }
    | TeamCommand::DeliveryReport { .. }
    | TeamCommand::WorkReport(_) => None,
  }
}

/// Refusals that commit nothing and pass: the operation stays open to be sent again under its id.
pub(crate) const fn retryable(code: AuthorityErrorCode) -> bool {
  matches!(
    code,
    AuthorityErrorCode::Unauthenticated
      | AuthorityErrorCode::RateLimited
      | AuthorityErrorCode::Unavailable
      | AuthorityErrorCode::UnsupportedContract
  )
}

/// The SHA-256 of the request's expected revisions and command, as JSON: what one operation id is bound to.
fn digest(body: &CommandRequest<TeamCommand>) -> ContentHash {
  let bytes = serde_json::to_vec(&(&body.expected, &body.command)).expect("a command request serializes");
  let mut hex = String::with_capacity(64);
  for byte in Sha256::digest(bytes) {
    write!(hex, "{byte:02x}").expect("writing to a string cannot fail");
  }
  ContentHash::new(&hex).expect("a SHA-256 in lowercase hex is a content hash")
}

/// The recorded JSON `text` of `operation` as `T`.
fn read<T: serde::de::DeserializeOwned>(operation: OperationId, text: &str) -> Result<T, ActionError> {
  serde_json::from_str(text).map_err(|_| ActionError::Unreadable(operation))
}
