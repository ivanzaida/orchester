//! The work reports of team runs (S9b, the server's `docs/work-reports.md`). Each change of a run's outcome is queued
//! once, persisted before it is sent, with its event id, the run's next sequence and its own operation id; then the
//! queued reports are sent in sequence order, each again with the same bytes until the server acknowledges it. The
//! outcomes are core's ([`ExecutionOutcome::of_status`]): an execution success is `succeeded` and never marks the task
//! Done, a failure keeps the claim, a cancellation releases an independent claim, and a reached limit is a pause the
//! work resumes from; a wait for a person has none (D1, D8).

use orchester_api::v2::{
  AuthorityErrorCode, AuthorityRef, CommandRequest, ExpectedRevisions, Subject, TeamCommand, TeamCommandValue,
  WorkReport,
};
use orchester_core::{
  id::{OperationId, RunId, WorkEventId},
  run::ExecutionOutcome,
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{AcceptedWork, NewWorkReport, ProjectDb, TeamFence, TeamRun, WorkReportEntry, WorkReportState};

use super::{Answer, TeamRunner, TeamWork, TeamWorkError, answer};

/// What one pass of reporting did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reported {
  /// Reports queued for outcomes that changed since the last pass.
  pub queued: usize,
  pub acknowledged: usize,
  /// Reports the server refused, kept as it refused them.
  pub refused: usize,
  /// Runs the server's answers fenced in this pass.
  pub fenced: Vec<RunId>,
  /// Whether reports wait for a server that cannot be reached or refused for now.
  pub waiting: bool,
}

impl<R: TeamRunner> TeamWork<R> {
  /// Queues a report of each team run whose outcome changed, then sends every queued report in order. A run already
  /// started keeps executing whatever the server answers; only its reports wait.
  pub async fn report(&self) -> Result<Reported, TeamWorkError> {
    let mut reported = Reported {
      queued: self.queue()?,
      ..Reported::default()
    };
    let runs = self.db()?.team_runs()?;
    for run in runs.into_iter().filter(|run| run.fence.is_none()) {
      self.send(run.run, &mut reported).await?;
      if reported.waiting {
        break;
      }
    }
    Ok(reported)
  }

  /// Queues a report of each unfenced team run whose outcome is not the one its last report carried.
  fn queue(&self) -> Result<usize, TeamWorkError> {
    let mut db = self.db()?;
    let mut queued = 0;
    for team in db.team_runs()? {
      if team.fence.is_some() {
        continue;
      }
      let run = db
        .run(team.run)?
        .ok_or(orchester_store::StoreError::RunNotFound(team.run))?;
      let Some(outcome) = ExecutionOutcome::of_status(run.status()) else {
        continue;
      };
      if queue_report(&mut db, &team, outcome)? {
        queued += 1;
      }
    }
    Ok(queued)
  }

  /// Sends the queued reports of `run` in order, until one waits or the run is fenced.
  async fn send(&self, run: RunId, reported: &mut Reported) -> Result<(), TeamWorkError> {
    let pending: Vec<WorkReportEntry> = self
      .db()?
      .work_reports(run)?
      .into_iter()
      .filter(|report| report.state == WorkReportState::Pending)
      .collect();
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    for report in pending {
      let body: CommandRequest<TeamCommand> =
        serde_json::from_str(&report.request).map_err(|_| TeamWorkError::UnreadableRun(run))?;
      let answered = session.authority().command(&body).await;
      let mut db = self.db()?;
      match answered {
        Ok(receipt) if receipt.operation_id == report.operation => {
          let TeamCommandValue::Work { work, .. } = receipt.result else {
            return Err(TeamWorkError::UnreadableRun(run));
          };
          let accepted = AcceptedWork {
            // The contract's types serialize to JSON without fail.
            work: serde_json::to_string(&work).expect("a work serializes"),
            revision: work.revision.get(),
          };
          db.acknowledge_work_report(report.event, &accepted, Timestamp::now())?;
          reported.acknowledged += 1;
          // The server keeps a report of a run older than the work's newest and changes nothing with it.
          if work.runs.last() != Some(&run) {
            db.fence_team_run(run, TeamFence::Superseded, Timestamp::now())?;
            reported.fenced.push(run);
            return Ok(());
          }
        }
        Ok(_) => return Err(TeamWorkError::UnreadableRun(run)),
        Err(error) => match answer(error) {
          Answer::Unreachable => {
            reported.waiting = true;
            return Ok(());
          }
          Answer::Refused(error) => {
            let fence = fence_of(error.code());
            let text = serde_json::to_string(&error).expect("a refusal serializes");
            db.refuse_work_report(report.event, &text, fence, Timestamp::now())?;
            reported.refused += 1;
            if fence.is_some() {
              reported.fenced.push(run);
              return Ok(());
            }
          }
        },
      }
    }
    Ok(())
  }
}

/// Queues the report of `outcome` for `team`; whether it was queued (not when the run's last report carried it).
fn queue_report(db: &mut ProjectDb, team: &TeamRun, outcome: ExecutionOutcome) -> Result<bool, TeamWorkError> {
  let text = serde_json::to_string(&outcome).expect("an outcome serializes");
  let authority = AuthorityRef {
    subject: Subject::Task { id: team.team_task },
    claim_id: team.claim,
    work_id: Some(team.work),
  };
  let run = team.run;
  let queued = db.queue_work_report(run, &text, Timestamp::now(), |sequence| {
    let (event, operation) = (WorkEventId::generate(), OperationId::generate());
    let body = CommandRequest {
      operation_id: operation,
      expected: ExpectedRevisions::default(),
      command: TeamCommand::WorkReport(WorkReport {
        authority,
        run_id: run,
        event_id: event,
        sequence,
        outcome,
        artifacts: Vec::new(),
      }),
    };
    NewWorkReport {
      event,
      operation,
      request: serde_json::to_string(&body).expect("a command request serializes"),
    }
  })?;
  Ok(queued.is_some())
}

/// The fence a refusal sets: the server's answers that the run's authority is gone (`docs/work-reports.md`,
/// "Fencing", and the membership check before it).
const fn fence_of(code: AuthorityErrorCode) -> Option<TeamFence> {
  match code {
    AuthorityErrorCode::ClaimLost => Some(TeamFence::ClaimLost),
    AuthorityErrorCode::MembershipRevoked => Some(TeamFence::MembershipRevoked),
    AuthorityErrorCode::SessionMismatch => Some(TeamFence::SessionMismatch),
    _ => None,
  }
}
