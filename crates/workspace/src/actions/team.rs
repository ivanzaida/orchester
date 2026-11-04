//! A team action's operation (S4): its id and request are recorded before the request is sent, so a response that is
//! lost leaves an open operation, never a guess. An open operation is first looked up on the server, then sent again
//! with the same id and the same bytes only when the server has no outcome of it, so a retry never commits twice. A
//! committed receipt is kept before its local effects are made, and a refusal is kept as it came; the same intent sent
//! again reads that back. The request's digest binds the id to the request: another request under the same id is
//! refused here, and by the server too.

use std::fmt::Write as _;

use orchester_api::{
  client::AuthorityCallError,
  v2::{AuthorityError, AuthorityErrorCode, CommandRequest, OperationRead, Subject, TeamCommand},
};
use orchester_core::{
  blob::ContentHash,
  id::{ModuleId, OperationId, TaskId},
  time::Timestamp,
};
use orchester_store::{NewTeamOperation, ProjectDb, TeamOperation, TeamOperationState, TeamSettlement};
use sha2::{Digest as _, Sha256};

use super::{ActionError, Current, Receipt, TeamOutcome, command::TeamRequest, effects, ownership::TeamLink};
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
    self
      .db
      .begin_team_operation(new, Timestamp::now())
      .map_err(ActionError::of_record)?;
    self.send(&body).await
  }

  /// Carries on with the recorded operation `record` from where it stands.
  pub(super) async fn resume(&mut self, record: TeamOperation) -> Result<TeamOutcome, ActionError> {
    let operation = record.id;
    match record.state {
      TeamOperationState::Sent => self.look_up(&record).await,
      TeamOperationState::Committed | TeamOperationState::EffectFailed => {
        let text = record.receipt.ok_or(ActionError::Unreadable(operation))?;
        let receipt = read(operation, &text)?;
        self.committed(receipt, &text)
      }
      TeamOperationState::Completed => {
        let text = record.receipt.ok_or(ActionError::Unreadable(operation))?;
        Ok(TeamOutcome::Completed {
          receipt: Box::new(read(operation, &text)?),
        })
      }
      TeamOperationState::Rejected => {
        let text = record.rejection.ok_or(ActionError::Unreadable(operation))?;
        Ok(TeamOutcome::Rejected {
          operation,
          error: read(operation, &text)?,
          retryable: false,
          current: None,
        })
      }
    }
  }

  /// What the server made of an operation sent before; sent again when the server has no outcome of it.
  async fn look_up(&mut self, record: &TeamOperation) -> Result<TeamOutcome, ActionError> {
    self.ensure_session()?;
    match self.session.authority().operation(record.id).await {
      Ok(OperationRead::Committed { response, .. }) => self.receipt(record.id, response),
      Ok(OperationRead::Rejected { error, .. }) => {
        self.refused(record.id, &read(record.id, &record.request)?, error).await
      }
      Err(AuthorityCallError::Refused { error, .. }) if error.code() == AuthorityErrorCode::NotFound => {
        let body: CommandRequest<TeamCommand> = read(record.id, &record.request)?;
        self.send(&body).await
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
    self
      .db
      .settle_team_operation(operation, TeamSettlement::Committed { receipt: text }, Timestamp::now())?;
    let pending = match effects::apply(self.db, self.link, &receipt.result) {
      Ok(pending) => pending,
      Err(error) => {
        self.db.settle_team_operation(
          operation,
          TeamSettlement::EffectFailed { receipt: text },
          Timestamp::now(),
        )?;
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
    self
      .db
      .settle_team_operation(operation, TeamSettlement::Completed { receipt: text }, Timestamp::now())?;
    Ok(TeamOutcome::Completed {
      receipt: Box::new(receipt),
    })
  }

  /// The server's refusal of `body`: kept unless retryable, with the subject as it is now after a revision conflict.
  async fn refused(
    &mut self,
    operation: OperationId,
    body: &CommandRequest<TeamCommand>,
    error: AuthorityError,
  ) -> Result<TeamOutcome, ActionError> {
    let retryable = retryable(error.code());
    if !retryable {
      let text = serde_json::to_string(&error).expect("a refusal serializes");
      self.db.settle_team_operation(
        operation,
        TeamSettlement::Rejected { rejection: &text },
        Timestamp::now(),
      )?;
    }
    let current = match (error.code(), readable(&body.command)) {
      (AuthorityErrorCode::RevisionConflict, Some(subject)) => Some(self.current(subject).await),
      _ => None,
    };
    Ok(TeamOutcome::Rejected {
      operation,
      error,
      retryable,
      current,
    })
  }

  async fn current(&self, subject: Readable) -> Current {
    let authority = self.session.authority();
    let read = match subject {
      Readable::Task(task) => authority.task(task).await.map(|read| Current::Task(Box::new(read))),
      Readable::Module(module) => authority
        .module(module)
        .await
        .map(|summary| Current::Module(Box::new(summary.module))),
      Readable::Policy => authority
        .workspace()
        .await
        .map(|workspace| Current::Policy(workspace.policy)),
    };
    read.unwrap_or_else(Current::NotRead)
  }
}

/// A subject whose current value the server reads after a revision conflict.
#[derive(Clone, Copy, Debug)]
enum Readable {
  Task(TaskId),
  Module(ModuleId),
  Policy,
}

/// The revision-checked subject of `command`.
const fn readable(command: &TeamCommand) -> Option<Readable> {
  match command {
    TeamCommand::TaskPatch { task_id, .. } => Some(Readable::Task(*task_id)),
    TeamCommand::ModulePatch { module_id, .. } | TeamCommand::ModuleBranch { module_id, .. } => {
      Some(Readable::Module(*module_id))
    }
    TeamCommand::AssignmentSet {
      subject: Subject::Task { id },
      ..
    } => Some(Readable::Task(*id)),
    TeamCommand::AssignmentSet {
      subject: Subject::Module { id },
      ..
    } => Some(Readable::Module(*id)),
    TeamCommand::PolicySet { .. } => Some(Readable::Policy),
    _ => None,
  }
}

/// Refusals that commit nothing and pass: the operation stays open to be sent again under its id.
const fn retryable(code: AuthorityErrorCode) -> bool {
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
