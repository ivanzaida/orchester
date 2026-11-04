//! Team starts (`schema::team_work`): a `work.start` of a team task, recorded by its operation before the command is
//! sent and carried forward one boundary at a time, so a crash at any of them goes on from it once and a retry never
//! makes a second work or run. The request is what the caller read and chose here, as JSON the workspace writes and
//! reads; the store keeps it as it is and holds the order of the boundaries.

use orchester_core::{
  blob::ContentHash,
  id::{OperationId, RunId, TaskId, WorkId},
  run::inputs::{InputsOrigin, RunInputs},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{ProjectDb, blobs, run_inputs, task_identities};
use crate::{
  RunStoreError, StoreError,
  rows::{corrupt, parse},
};

/// The media type the files of an execution bundle are kept as: their bytes as the server pinned them.
const BUNDLE_FILE: &str = "application/octet-stream";

/// Where a team start stands on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamStartStage {
  /// Recorded before `work.start` was sent: whether the server made the work is not known yet.
  Requested,
  /// The receipt's work and the reference of the bundle it runs are kept; the bundle is not.
  Accepted,
  /// The bundle's inputs and files are kept as the run's accepted inputs; the runner has not recorded the run yet.
  InputsStored,
  /// The runner recorded and started the run.
  Started,
  /// The server refused the start: no work was made.
  Refused,
}

impl TeamStartStage {
  const fn text(self) -> &'static str {
    match self {
      Self::Requested => "requested",
      Self::Accepted => "accepted",
      Self::InputsStored => "inputs_stored",
      Self::Started => "started",
      Self::Refused => "refused",
    }
  }

  fn parse(text: &str) -> Result<Self, StoreError> {
    Ok(match text {
      "requested" => Self::Requested,
      "accepted" => Self::Accepted,
      "inputs_stored" => Self::InputsStored,
      "started" => Self::Started,
      "refused" => Self::Refused,
      other => return Err(corrupt("team start stage", other)),
    })
  }
}

/// A recorded team start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamStart {
  pub operation: OperationId,
  /// This project's identity of the team task.
  pub task: TaskId,
  pub request: String,
  pub stage: TeamStartStage,
  /// The work the receipt names and the hash of the bundle it runs, from [`TeamStartStage::Accepted`] on.
  pub work: Option<(WorkId, ContentHash)>,
  /// The run the runner started, at [`TeamStartStage::Started`].
  pub run: Option<RunId>,
  /// The server's refusal, as JSON, at [`TeamStartStage::Refused`].
  pub refusal: Option<String>,
}

impl ProjectDb {
  /// Records the start of team task `task` under `operation` with `request` before `work.start` is sent, and returns
  /// it. The same operation recorded before with the same task and request returns that record, wherever it stands;
  /// with another one it is [`StoreError::OperationMismatch`].
  pub fn begin_team_start(
    &mut self,
    (operation, task): (OperationId, TaskId),
    request: &str,
    at: Timestamp,
  ) -> Result<TeamStart, StoreError> {
    let transaction = self.connection.transaction()?;
    if let Some(known) = load(&transaction, operation)? {
      if known.task != task || known.request != request {
        return Err(StoreError::OperationMismatch(operation));
      }
      return Ok(known);
    }
    task_identities::ensure_known(&transaction, task)?;
    transaction.execute(
      "INSERT INTO team_starts (operation_id, task_id, request, stage, created_at, updated_at) \
       VALUES (?1, ?2, ?3, 'requested', ?4, ?4)",
      params![operation.to_string(), task.to_string(), request, at.unix_millis()],
    )?;
    let recorded = load(&transaction, operation)?.ok_or(StoreError::TeamStartNotFound(operation))?;
    transaction.commit()?;
    Ok(recorded)
  }

  pub fn team_start(&self, operation: OperationId) -> Result<Option<TeamStart>, StoreError> {
    load(&self.connection, operation)
  }

  /// The starts not started and not refused yet, oldest first: what recovery carries on with.
  pub fn open_team_starts(&self) -> Result<Vec<TeamStart>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT} WHERE stage IN ('requested', 'accepted', 'inputs_stored') ORDER BY created_at, operation_id"
    ))?;
    let rows = statement.query_map([], read)?;
    rows.map(|row| row?).collect()
  }

  /// Keeps the work the receipt of `operation` names and its bundle's hash, before anything else is done with them.
  /// Keeping the same again changes nothing; another work is [`StoreError::OperationMismatch`].
  pub fn accept_team_start(
    &mut self,
    operation: OperationId,
    (work, bundle): (WorkId, &ContentHash),
    at: Timestamp,
  ) -> Result<TeamStart, StoreError> {
    let transaction = self.connection.transaction()?;
    let known = load(&transaction, operation)?.ok_or(StoreError::TeamStartNotFound(operation))?;
    match (&known.stage, &known.work) {
      (TeamStartStage::Requested, _) => {
        transaction.execute(
          "UPDATE team_starts SET stage = 'accepted', work_id = ?2, bundle_sha256 = ?3, updated_at = ?4 \
           WHERE operation_id = ?1",
          params![
            operation.to_string(),
            work.to_string(),
            bundle.as_str(),
            at.unix_millis()
          ],
        )?;
      }
      (_, Some((kept, hash))) if *kept == work && hash == bundle => return Ok(known),
      (TeamStartStage::Refused, _) => return Err(out_of_order(&known)),
      _ => return Err(StoreError::OperationMismatch(operation)),
    }
    let accepted = load(&transaction, operation)?.ok_or(StoreError::TeamStartNotFound(operation))?;
    transaction.commit()?;
    Ok(accepted)
  }

  /// Records the server's `refusal` of the start under `operation`, kept as it came: of `work.start`, which made no
  /// work, or of the bundle its receipt named, whose authority is gone. Only a start whose inputs are not kept yet is
  /// refused.
  pub fn refuse_team_start(&mut self, operation: OperationId, refusal: &str, at: Timestamp) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE team_starts SET stage = 'refused', refusal = ?2, updated_at = ?3 WHERE operation_id = ?1 \
       AND stage IN ('requested', 'accepted')",
      params![operation.to_string(), refusal, at.unix_millis()],
    )?;
    ensure_updated(&self.connection, operation, updated)
  }

  /// Keeps the inputs of the accepted start under `operation` as the accepted inputs of its run, with the bundle's
  /// `files` as blobs, in one transaction with the start's own record. The files are written first, as every blob is;
  /// a crash before the transaction leaves only files no row names, which the next attempt writes over unchanged.
  pub fn store_team_start_inputs(
    &mut self,
    operation: OperationId,
    inputs: &RunInputs,
    files: &[Vec<u8>],
    at: Timestamp,
  ) -> Result<(), RunStoreError> {
    let known = load(&self.connection, operation)?.ok_or(StoreError::TeamStartNotFound(operation))?;
    if !accepted_by(&known, inputs) {
      return Err(RunStoreError::InputsMismatch {
        task: known.task,
        reason: "the inputs are not the ones the start's receipt accepted",
      });
    }
    let written = files
      .iter()
      .map(|content| blobs::write(&self.directory, content))
      .collect::<Result<Vec<_>, _>>()?;
    let transaction = self.connection.transaction()?;
    for blob in &written {
      blob.record(&transaction, BUNDLE_FILE, at)?;
    }
    run_inputs::keep_accepted(&transaction, known.task, inputs, at)?;
    advance(
      &transaction,
      operation,
      TeamStartStage::Accepted,
      TeamStartStage::InputsStored,
      at,
    )?;
    transaction.commit()?;
    Ok(())
  }
}

/// Whether `inputs` are those the receipt `start` kept accepted: its operation's, for its work and bundle.
fn accepted_by(start: &TeamStart, inputs: &RunInputs) -> bool {
  let InputsOrigin::Team(accepted) = inputs.bundle().origin() else {
    return false;
  };
  accepted.operation == start.operation
    && start
      .work
      .as_ref()
      .is_some_and(|(work, bundle)| *work == accepted.work && *bundle == accepted.bundle)
}

/// Moves the start under `operation` from `from` to `to`, which only a start at `from` does.
fn advance(
  connection: &Connection,
  operation: OperationId,
  from: TeamStartStage,
  to: TeamStartStage,
  at: Timestamp,
) -> Result<(), StoreError> {
  let updated = connection.execute(
    "UPDATE team_starts SET stage = ?3, updated_at = ?4 WHERE operation_id = ?1 AND stage = ?2",
    params![operation.to_string(), from.text(), to.text(), at.unix_millis()],
  )?;
  ensure_updated(connection, operation, updated)
}

/// Marks the start under `operation`, whose inputs are stored, started with the runner's run `run`.
pub(super) fn mark_started(
  connection: &Connection,
  operation: OperationId,
  run: RunId,
  at: Timestamp,
) -> Result<(), StoreError> {
  let updated = connection.execute(
    "UPDATE team_starts SET stage = 'started', run_id = ?2, updated_at = ?3 WHERE operation_id = ?1 \
     AND stage = 'inputs_stored'",
    params![operation.to_string(), run.to_string(), at.unix_millis()],
  )?;
  ensure_updated(connection, operation, updated)
}

/// The start out of order when `updated` rows say the update found it elsewhere.
fn ensure_updated(connection: &Connection, operation: OperationId, updated: usize) -> Result<(), StoreError> {
  if updated == 1 {
    return Ok(());
  }
  let known = load(connection, operation)?.ok_or(StoreError::TeamStartNotFound(operation))?;
  Err(out_of_order(&known))
}

fn out_of_order(start: &TeamStart) -> StoreError {
  StoreError::TeamStartStage {
    operation: start.operation,
    stage: start.stage.text(),
  }
}

const SELECT: &str =
  "SELECT operation_id, task_id, request, stage, work_id, bundle_sha256, run_id, refusal FROM team_starts";

pub(super) fn load(connection: &Connection, operation: OperationId) -> Result<Option<TeamStart>, StoreError> {
  connection
    .query_row(
      &format!("{SELECT} WHERE operation_id = ?1"),
      [operation.to_string()],
      read,
    )
    .optional()?
    .transpose()
}

/// A row as a start; a value that breaks the rules is [`StoreError::Corrupt`].
fn read(row: &Row<'_>) -> rusqlite::Result<Result<TeamStart, StoreError>> {
  let (operation, task, request, stage): (String, String, String, String) =
    (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
  let (work, bundle, run, refusal): (Option<String>, Option<String>, Option<String>, Option<String>) =
    (row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?);
  Ok((|| {
    let work = match (work, bundle) {
      (Some(work), Some(bundle)) => Some((
        parse("team start work", &work)?,
        ContentHash::new(&bundle).map_err(|error| corrupt("team start bundle", error))?,
      )),
      (None, None) => None,
      _ => return Err(corrupt("team start work", "a work without its bundle")),
    };
    Ok(TeamStart {
      operation: parse("team start operation", &operation)?,
      task: parse("team start task", &task)?,
      request,
      stage: TeamStartStage::parse(&stage)?,
      work,
      run: run.map(|run| parse("team start run", &run)).transpose()?,
      refusal,
    })
  })())
}
