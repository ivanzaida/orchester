//! Team runs and their work report outbox (`schema::team_work`). A run the runner started from a team start reports
//! under the exact authority the start's receipt gave it. Each report is queued once, with its event id, the next
//! sequence of its run, its operation id and its request, before it is sent; it is sent again with the same bytes until
//! the server acknowledges or refuses it. The server's acceptance of the run's work is kept apart from the run's local
//! status, and a fence, once set, stops the run's reporting for good. Outcomes, requests, refusals and accepted works
//! are JSON the workspace writes and reads; the store keeps them as they are and holds the order.

use orchester_core::{
  id::{ClaimId, OperationId, RunId, TaskId, WorkEventId, WorkId},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{
  ProjectDb, run_inputs,
  team_starts::{self, TeamStartStage},
};
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

/// Why a team run reports nothing more: the server's answer that its authority is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamFence {
  /// The claim was taken over, released or unassigned, or the work is no longer its latest (`claim_lost`).
  ClaimLost,
  /// The person is no longer a member of the workspace (`membership_revoked`).
  MembershipRevoked,
  /// The claim is held by the same account from another session (`session_mismatch`).
  SessionMismatch,
  /// A newer run carries the work on: the server keeps this run's reports and changes nothing with them.
  Superseded,
}

impl TeamFence {
  const fn text(self) -> &'static str {
    match self {
      Self::ClaimLost => "claim_lost",
      Self::MembershipRevoked => "membership_revoked",
      Self::SessionMismatch => "session_mismatch",
      Self::Superseded => "superseded",
    }
  }

  fn parse(text: &str) -> Result<Self, StoreError> {
    Ok(match text {
      "claim_lost" => Self::ClaimLost,
      "membership_revoked" => Self::MembershipRevoked,
      "session_mismatch" => Self::SessionMismatch,
      "superseded" => Self::Superseded,
      other => return Err(corrupt("team run fence", other)),
    })
  }
}

/// The work as the server last answered one of the run's reports, and its revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedWork {
  pub work: String,
  pub revision: u64,
}

/// A run started through the team server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRun {
  pub run: RunId,
  pub operation: OperationId,
  /// The server's id of the task, the claim the work started under and the work.
  pub team_task: TaskId,
  pub claim: ClaimId,
  pub work: WorkId,
  /// The outcome of the last report queued, as JSON.
  pub last_outcome: Option<String>,
  pub accepted: Option<AcceptedWork>,
  pub fence: Option<TeamFence>,
}

/// Where a queued report stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkReportState {
  /// Queued, and sent until the server answers it.
  Pending,
  Acknowledged,
  /// The server refused it; the refusal is kept.
  Refused,
}

/// A report about to be queued, made for its sequence.
#[derive(Clone, Debug)]
pub struct NewWorkReport {
  pub event: WorkEventId,
  pub operation: OperationId,
  pub request: String,
}

/// A queued report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkReportEntry {
  pub event: WorkEventId,
  pub run: RunId,
  pub sequence: u64,
  pub operation: OperationId,
  pub outcome: String,
  pub request: String,
  pub state: WorkReportState,
  pub refusal: Option<String>,
}

impl ProjectDb {
  /// Records the run `run` the runner started for the team start under `operation`, under the authority its accepted
  /// inputs carry, and marks the start started, in one transaction. The same run again returns it; another run is
  /// [`StoreError::OperationMismatch`].
  pub fn record_team_start_run(
    &mut self,
    operation: OperationId,
    run: RunId,
    at: Timestamp,
  ) -> Result<TeamRun, StoreError> {
    let transaction = self.connection.transaction()?;
    let start = team_starts::load(&transaction, operation)?.ok_or(StoreError::TeamStartNotFound(operation))?;
    match start.stage {
      TeamStartStage::Started if start.run == Some(run) => {
        return load_by_run(&transaction, run)?.ok_or(StoreError::TeamRunNotFound(run));
      }
      TeamStartStage::Started => return Err(StoreError::OperationMismatch(operation)),
      _ => {}
    }
    team_starts::mark_started(&transaction, operation, run, at)?;
    let accepted = run_inputs::team_acceptance(&transaction, operation)?;
    let origin = accepted.ok_or(StoreError::TeamStartStage {
      operation,
      stage: "without accepted inputs",
    })?;
    transaction.execute(
      "INSERT INTO team_runs (run_id, operation_id, team_task_id, claim_id, work_id, next_sequence) \
       VALUES (?1, ?2, ?3, ?4, ?5, 1)",
      params![
        run.to_string(),
        operation.to_string(),
        origin.task.task.to_string(),
        origin.claim.to_string(),
        origin.work.to_string()
      ],
    )?;
    let recorded = load_by_run(&transaction, run)?.ok_or(StoreError::TeamRunNotFound(run))?;
    transaction.commit()?;
    Ok(recorded)
  }

  pub fn team_run(&self, run: RunId) -> Result<Option<TeamRun>, StoreError> {
    load_by_run(&self.connection, run)
  }

  /// Every team run of the project, oldest start first.
  pub fn team_runs(&self) -> Result<Vec<TeamRun>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT_RUN} JOIN team_starts AS starts ON starts.operation_id = runs.operation_id \
       ORDER BY starts.created_at, runs.run_id"
    ))?;
    let rows = statement.query_map([], read_run)?;
    rows.map(|row| row?).collect()
  }

  /// Queues a report of `outcome` for team run `run`, made by `make` for its sequence, unless the run is fenced or
  /// `outcome` is the one its last report carried. The queued report; `None` when nothing was queued.
  pub fn queue_work_report(
    &mut self,
    run: RunId,
    outcome: &str,
    at: Timestamp,
    make: impl FnOnce(u64) -> NewWorkReport,
  ) -> Result<Option<WorkReportEntry>, StoreError> {
    let transaction = self.connection.transaction()?;
    let known = load_by_run(&transaction, run)?.ok_or(StoreError::TeamRunNotFound(run))?;
    if known.fence.is_some() || known.last_outcome.as_deref() == Some(outcome) {
      return Ok(None);
    }
    let sequence: i64 = transaction.query_row(
      "SELECT next_sequence FROM team_runs WHERE run_id = ?1",
      [run.to_string()],
      |row| row.get(0),
    )?;
    let new = make(u64::try_from(sequence).map_err(|_| corrupt("team run sequence", sequence))?);
    transaction.execute(
      "INSERT INTO work_report_outbox (event_id, run_id, sequence, operation_id, outcome, request, state, created_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)",
      params![
        new.event.to_string(),
        run.to_string(),
        sequence,
        new.operation.to_string(),
        outcome,
        new.request,
        at.unix_millis()
      ],
    )?;
    transaction.execute(
      "UPDATE team_runs SET next_sequence = next_sequence + 1, last_outcome = ?2 WHERE run_id = ?1",
      params![run.to_string(), outcome],
    )?;
    let queued = load_report(&transaction, new.event)?;
    transaction.commit()?;
    Ok(queued)
  }

  /// Every report queued for team run `run`, in sequence order.
  pub fn work_reports(&self, run: RunId) -> Result<Vec<WorkReportEntry>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{SELECT_REPORT} WHERE run_id = ?1 ORDER BY sequence"))?;
    let rows = statement.query_map([run.to_string()], read_report)?;
    rows.map(|row| row?).collect()
  }

  /// Records that the server accepted report `event`, and keeps the work it answered with as the run's acceptance
  /// unless the run keeps a newer revision of it.
  pub fn acknowledge_work_report(
    &mut self,
    event: WorkEventId,
    accepted: &AcceptedWork,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let report = load_report(&transaction, event)?.ok_or_else(|| corrupt("work report", "not queued"))?;
    settle(&transaction, event, None, at)?;
    transaction.execute(
      "UPDATE team_runs SET accepted_work = ?2, accepted_revision = ?3 WHERE run_id = ?1 \
       AND (accepted_revision IS NULL OR accepted_revision < ?3)",
      params![
        report.run.to_string(),
        accepted.work,
        tokens_column("work revision", accepted.revision)?
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// Records the server's refusal of report `event`, kept as it came, and fences its run with `fence` when the refusal
  /// says its authority is gone.
  pub fn refuse_work_report(
    &mut self,
    event: WorkEventId,
    refusal: &str,
    fence: Option<TeamFence>,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let report = load_report(&transaction, event)?.ok_or_else(|| corrupt("work report", "not queued"))?;
    settle(&transaction, event, Some(refusal), at)?;
    if let Some(fence) = fence {
      fence_run(&transaction, report.run, fence, at)?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// Fences team run `run`: it reports nothing more. A run fenced already keeps its first fence.
  pub fn fence_team_run(&mut self, run: RunId, fence: TeamFence, at: Timestamp) -> Result<(), StoreError> {
    fence_run(&self.connection, run, fence, at)
  }
}

fn fence_run(connection: &Connection, run: RunId, fence: TeamFence, at: Timestamp) -> Result<(), StoreError> {
  let updated = connection.execute(
    "UPDATE team_runs SET fence = ?2, fenced_at = ?3 WHERE run_id = ?1 AND fence IS NULL",
    params![run.to_string(), fence.text(), at.unix_millis()],
  )?;
  if updated == 0 && load_by_run(connection, run)?.is_none() {
    return Err(StoreError::TeamRunNotFound(run));
  }
  Ok(())
}

fn settle(connection: &Connection, event: WorkEventId, refusal: Option<&str>, at: Timestamp) -> Result<(), StoreError> {
  let state = if refusal.is_some() { "refused" } else { "acknowledged" };
  connection.execute(
    "UPDATE work_report_outbox SET state = ?2, refusal = ?3, settled_at = ?4 WHERE event_id = ?1 AND state = 'pending'",
    params![event.to_string(), state, refusal, at.unix_millis()],
  )?;
  Ok(())
}

const SELECT_RUN: &str = "SELECT runs.run_id, runs.operation_id, runs.team_task_id, runs.claim_id, runs.work_id, \
                          runs.last_outcome, runs.accepted_work, runs.accepted_revision, runs.fence FROM team_runs AS runs";

fn load_by_run(connection: &Connection, run: RunId) -> Result<Option<TeamRun>, StoreError> {
  connection
    .query_row(
      &format!("{SELECT_RUN} WHERE runs.run_id = ?1"),
      [run.to_string()],
      read_run,
    )
    .optional()?
    .transpose()
}

fn read_run(row: &Row<'_>) -> rusqlite::Result<Result<TeamRun, StoreError>> {
  let ids: (String, String, String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
  let (last_outcome, accepted_work, accepted_revision, fence): (
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
  ) = (row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?);
  Ok((|| {
    let accepted = match (accepted_work, accepted_revision) {
      (Some(work), Some(revision)) => Some(AcceptedWork {
        work,
        revision: u64::try_from(revision).map_err(|_| corrupt("accepted work revision", revision))?,
      }),
      (None, None) => None,
      _ => return Err(corrupt("accepted work", "a work without its revision")),
    };
    Ok(TeamRun {
      run: parse("team run", &ids.0)?,
      operation: parse("team run operation", &ids.1)?,
      team_task: parse("team run task", &ids.2)?,
      claim: parse("team run claim", &ids.3)?,
      work: parse("team run work", &ids.4)?,
      last_outcome,
      accepted,
      fence: fence.as_deref().map(TeamFence::parse).transpose()?,
    })
  })())
}

const SELECT_REPORT: &str = "SELECT event_id, run_id, sequence, operation_id, outcome, request, state, refusal FROM \
                             work_report_outbox";

fn load_report(connection: &Connection, event: WorkEventId) -> Result<Option<WorkReportEntry>, StoreError> {
  connection
    .query_row(
      &format!("{SELECT_REPORT} WHERE event_id = ?1"),
      [event.to_string()],
      read_report,
    )
    .optional()?
    .transpose()
}

fn read_report(row: &Row<'_>) -> rusqlite::Result<Result<WorkReportEntry, StoreError>> {
  let (event, run, sequence, operation): (String, String, i64, String) =
    (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
  let (outcome, request, state, refusal): (String, String, String, Option<String>) =
    (row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?);
  Ok((|| {
    Ok(WorkReportEntry {
      event: parse("work report event", &event)?,
      run: parse("work report run", &run)?,
      sequence: u64::try_from(sequence).map_err(|_| corrupt("work report sequence", sequence))?,
      operation: parse("work report operation", &operation)?,
      outcome,
      request,
      state: match state.as_str() {
        "pending" => WorkReportState::Pending,
        "acknowledged" => WorkReportState::Acknowledged,
        "refused" => WorkReportState::Refused,
        other => return Err(corrupt("work report state", other)),
      },
      refusal,
    })
  })())
}
