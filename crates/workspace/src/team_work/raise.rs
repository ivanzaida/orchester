//! A team run's limits raised by its holder (OA-34): the team server is asked with `work.raise_limits` under the run's
//! exact authority before this computer raises the run, so the Admin's ceiling (the workspace policy's `limit_raises`)
//! holds for every raise, a person's or the Orchestrator's. The run's queued reports go first: the server takes a raise
//! of work it saw stop at a limit only. Nothing is kept here: a raise the server refused, or could not be asked about,
//! is not made.

use orchester_api::v2::{
  AuthorityError, AuthorityRef, CommandRequest, ExpectedRevisions, LimitRaise, Subject, TeamCommand, TeamCommandValue,
};
use orchester_core::id::{OperationId, RunId};
use orchester_store::StoreError;

use super::{Answer, TeamWork, TeamWorkError, answer};

/// What the team server said of a raise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaiseAnswer {
  /// The server keeps the raise: the run was raised `raises` times with it.
  Raised { raises: u32 },
  /// The server refused it, e.g. `decision_required [D6]` while the Admin allows no raise, or `invalid` beyond the
  /// ceiling.
  Refused(AuthorityError),
  /// The server cannot be reached, or refused for now: nothing was raised.
  Unreachable,
}

impl<R> TeamWork<R> {
  /// Asks the team server to raise team run `run`'s limits by `raise`, after the run's queued reports are sent.
  pub async fn raise_limits(&self, run: RunId, raise: LimitRaise) -> Result<RaiseAnswer, TeamWorkError> {
    let reported = self.report().await?;
    if reported.waiting {
      return Ok(RaiseAnswer::Unreachable);
    }
    let team = self.db()?.team_run(run)?.ok_or(StoreError::RunNotFound(run))?;
    let body = CommandRequest {
      operation_id: OperationId::generate(),
      expected: ExpectedRevisions::default(),
      command: TeamCommand::WorkRaiseLimits {
        authority: AuthorityRef {
          subject: Subject::Task { id: team.team_task },
          claim_id: team.claim,
          work_id: Some(team.work),
        },
        run_id: run,
        raise,
      },
    };
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    match session.authority().command(&body).await {
      Ok(receipt) => match receipt.result {
        TeamCommandValue::LimitsRaised { raises, .. } => Ok(RaiseAnswer::Raised { raises }),
        _ => Err(TeamWorkError::UnreadableRun(run)),
      },
      Err(error) => Ok(match answer(error) {
        Answer::Unreachable => RaiseAnswer::Unreachable,
        Answer::Refused(error) => RaiseAnswer::Refused(error),
      }),
    }
  }
}
