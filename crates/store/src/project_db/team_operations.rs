//! The team operations of a project linked to a team server (`schema::team_operations`): each command's id and request
//! are recorded before it is sent, and its outcome once known, so it is looked up or sent again with the same id after
//! a lost response or a restart. The request and the receipt are JSON the workspace writes and reads; the store keeps
//! them as they are and holds the rules that make an operation one intent: its request never changes, and a settled
//! outcome never changes.

use orchester_core::{
  blob::ContentHash,
  id::{OperationId, ProjectId, ServerId},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{ProjectDb, sharing};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// Where a team operation stands on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamOperationState {
  /// Recorded and sent, or about to be: whether the server committed it is not known yet.
  Sent,
  /// The server committed it; local effects of its receipt are still to happen.
  Committed,
  /// The server committed it and every local effect of its receipt happened.
  Completed,
  /// The server committed it and a local effect of its receipt failed; it stays committed.
  EffectFailed,
  /// The server refused it.
  Rejected,
}

impl TeamOperationState {
  const fn text(self) -> &'static str {
    match self {
      Self::Sent => "sent",
      Self::Committed => "committed",
      Self::Completed => "completed",
      Self::EffectFailed => "effect_failed",
      Self::Rejected => "rejected",
    }
  }

  fn parse(text: &str) -> Result<Self, StoreError> {
    Ok(match text {
      "sent" => Self::Sent,
      "committed" => Self::Committed,
      "completed" => Self::Completed,
      "effect_failed" => Self::EffectFailed,
      "rejected" => Self::Rejected,
      other => return Err(corrupt("team operation state", other)),
    })
  }
}

/// A team command about to be sent: its id, its kind (`task.patch`, …), the digest of its request and the request.
#[derive(Clone, Copy, Debug)]
pub struct NewTeamOperation<'a> {
  pub id: OperationId,
  pub kind: &'a str,
  pub digest: &'a ContentHash,
  pub request: &'a str,
}

/// A recorded team operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamOperation {
  pub id: OperationId,
  /// The server and its project the operation was recorded for: the project's link when it was sent, which a project
  /// made local and linked again elsewhere no longer has.
  pub server: ServerId,
  pub project: ProjectId,
  pub kind: String,
  pub digest: ContentHash,
  pub request: String,
  pub state: TeamOperationState,
  /// The committed command's receipt, from [`TeamOperationState::Committed`] on.
  pub receipt: Option<String>,
  /// The server's refusal of a [`TeamOperationState::Rejected`] operation.
  pub rejection: Option<String>,
}

/// What became of a sent operation.
#[derive(Clone, Copy, Debug)]
pub enum TeamSettlement<'a> {
  Committed { receipt: &'a str },
  Completed { receipt: &'a str },
  EffectFailed { receipt: &'a str },
  Rejected { rejection: &'a str },
}

impl ProjectDb {
  /// Records `new` before it is sent and returns it. The same id recorded before with the same digest returns that
  /// record, whatever became of it; with another digest it is [`StoreError::OperationMismatch`], and recorded for
  /// another server project than the one linked now [`StoreError::OtherServerProject`]. The project must be linked to
  /// a team server.
  pub fn begin_team_operation(
    &mut self,
    new: NewTeamOperation<'_>,
    at: Timestamp,
  ) -> Result<TeamOperation, StoreError> {
    let transaction = self.connection.transaction()?;
    let link = sharing::link(&transaction)?.ok_or(StoreError::NotShared)?;
    if let Some(known) = load(&transaction, new.id)? {
      if known.digest != *new.digest {
        return Err(StoreError::OperationMismatch(new.id));
      }
      if known.server != link.server || known.project != link.project {
        return Err(StoreError::OtherServerProject);
      }
      return Ok(known);
    }
    transaction.execute(
      "INSERT INTO team_operations (id, server_id, team_project_id, kind, digest, request, state, created_at, \
       updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'sent', ?7, ?7)",
      params![
        new.id.to_string(),
        link.server.to_string(),
        link.project.to_string(),
        new.kind,
        new.digest.as_str(),
        new.request,
        at.unix_millis()
      ],
    )?;
    let recorded = load(&transaction, new.id)?.ok_or(StoreError::OperationNotFound(new.id))?;
    transaction.commit()?;
    Ok(recorded)
  }

  pub fn team_operation(&self, id: OperationId) -> Result<Option<TeamOperation>, StoreError> {
    load(&self.connection, id)
  }

  /// The operations of the server project the project is linked to now whose outcome or local effects are still to be
  /// learned or made, oldest first; none while the project is local. Those of a link the project no longer has are
  /// left as they are: they are another server project's, never to be sent to this one.
  pub fn open_team_operations(&self) -> Result<Vec<TeamOperation>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT} WHERE state IN ('sent', 'committed', 'effect_failed') AND server_id = (SELECT server_id FROM \
       sharing) AND team_project_id = (SELECT project_id FROM sharing) ORDER BY created_at, id"
    ))?;
    let rows = statement.query_map([], read)?;
    rows.map(|row| row?).collect()
  }

  /// Records what became of operation `id`. A settled outcome stays: the database refuses to change it.
  pub fn settle_team_operation(
    &mut self,
    id: OperationId,
    settlement: TeamSettlement<'_>,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let (state, receipt, rejection) = match settlement {
      TeamSettlement::Committed { receipt } => (TeamOperationState::Committed, Some(receipt), None),
      TeamSettlement::Completed { receipt } => (TeamOperationState::Completed, Some(receipt), None),
      TeamSettlement::EffectFailed { receipt } => (TeamOperationState::EffectFailed, Some(receipt), None),
      TeamSettlement::Rejected { rejection } => (TeamOperationState::Rejected, None, Some(rejection)),
    };
    let updated = self.connection.execute(
      "UPDATE team_operations SET state = ?2, receipt = ?3, rejection = ?4, updated_at = ?5 WHERE id = ?1",
      params![id.to_string(), state.text(), receipt, rejection, at.unix_millis()],
    )?;
    if updated == 0 {
      return Err(StoreError::OperationNotFound(id));
    }
    Ok(())
  }
}

const SELECT: &str = "SELECT id, server_id, team_project_id, kind, digest, request, state, receipt, rejection FROM \
                      team_operations";

fn load(connection: &Connection, id: OperationId) -> Result<Option<TeamOperation>, StoreError> {
  connection
    .query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], read)
    .optional()?
    .transpose()
}

/// A row as an operation; a value that breaks the rules is [`StoreError::Corrupt`].
fn read(row: &Row<'_>) -> rusqlite::Result<Result<TeamOperation, StoreError>> {
  let (id, server, project, kind, digest, request): (String, String, String, String, String, String) = (
    row.get(0)?,
    row.get(1)?,
    row.get(2)?,
    row.get(3)?,
    row.get(4)?,
    row.get(5)?,
  );
  let (state, receipt, rejection): (String, Option<String>, Option<String>) = (row.get(6)?, row.get(7)?, row.get(8)?);
  Ok((|| {
    Ok(TeamOperation {
      id: parse("team operation id", &id)?,
      server: parse("team operation server", &server)?,
      project: parse("team operation project", &project)?,
      kind,
      digest: ContentHash::new(&digest).map_err(|error| corrupt("team operation digest", error))?,
      request,
      state: TeamOperationState::parse(&state)?,
      receipt,
      rejection,
    })
  })())
}
