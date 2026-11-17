//! The exact heartbeat of team runs (S5, OA-31): this computer keeps the claim of a team task for as long as it holds
//! work of it that the server still sees running. That is the run at work, and also one whose step waits for a person
//! (an approval, an answer, a plan), one stuck for a person's decision, one a person paused and one interrupted here:
//! none of those ends the server's work, which only a report does. Each claim is named once, by its exact authority
//! with no work (the server's `docs/claim-authority.md`: "`work_id` must be `null`"), and the server renews it while
//! its latest work runs. Work the server saw end or pause (succeeded and waiting to be merged, at a limit) is not
//! named: the server renews no such claim (D1, its `transition_denied`), and naming it would refuse the whole
//! heartbeat. `decision_required` is an outcome to show, never a failure to retry; an authority it lists as lost fences
//! its runs, which are then stopped ([`super::fenced`]).

use orchester_api::v2::{
  AuthorityError, AuthorityErrorCode, AuthorityRef, DecisionId, ErrorDetails, ExactHeartbeat, ExecutionState,
  MAX_HEARTBEAT_AUTHORITIES, Subject, WorkRead,
};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_store::{TeamFence, TeamRun};

use super::{Answer, TeamWork, TeamWorkError, answer};

/// What one heartbeat did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heartbeat {
  /// No team run here holds work the server sees running: nothing was sent.
  NothingToRenew,
  /// The claims of `renewed` runs are renewed; those of `lost` runs are gone, and the runs are fenced.
  Renewed { renewed: Vec<RunId>, lost: Vec<RunId> },
  /// The server renews nothing while the work of a named claim waits for a person or for review: the decisions it
  /// names are open (D1 for a task's).
  DecisionRequired(Vec<DecisionId>),
  /// The server renews nothing: a named claim's work is not running there.
  NotRunning,
  /// The server cannot be reached, or refused for now; the next heartbeat tries again.
  Unreachable,
  /// The server refused the heartbeat for another reason, kept as it came.
  Refused(AuthorityError),
}

/// A claim to renew and the runs here that hold work under it.
type Held = (AuthorityRef, Vec<RunId>);

impl<R> TeamWork<R> {
  /// Renews the claims of the team runs whose work the server sees running, at most [`MAX_HEARTBEAT_AUTHORITIES`] at a
  /// time.
  pub async fn heartbeat(&self) -> Result<Heartbeat, TeamWorkError> {
    let held = self.held()?;
    if held.is_empty() {
      return Ok(Heartbeat::NothingToRenew);
    }
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    let (mut renewed, mut lost) = (Vec::new(), Vec::new());
    for chunk in held.chunks(MAX_HEARTBEAT_AUTHORITIES) {
      let heartbeat = ExactHeartbeat {
        authorities: chunk.iter().map(|(authority, _)| *authority).collect(),
      };
      let answered = match session.authority().heartbeat(&heartbeat).await {
        Ok(answered) => answered,
        Err(error) => return Ok(refused(answer(error))),
      };
      for (authority, runs) in chunk {
        if answered.lost.contains(authority) {
          let mut db = self.db()?;
          for run in runs {
            db.fence_team_run(*run, TeamFence::ClaimLost, Timestamp::now())?;
          }
          lost.extend(runs);
        } else {
          renewed.extend(runs);
        }
      }
    }
    Ok(Heartbeat::Renewed { renewed, lost })
  }

  /// The claims of the unfenced team runs whose work the server sees running, each once, with the runs under it.
  fn held(&self) -> Result<Vec<Held>, TeamWorkError> {
    let mut held: Vec<Held> = Vec::new();
    for team in self.db()?.team_runs()? {
      if team.fence.is_some() || !server_runs(&team)? {
        continue;
      }
      let authority = AuthorityRef {
        subject: Subject::Task { id: team.team_task },
        claim_id: team.claim,
        work_id: None,
      };
      match held.iter_mut().find(|(known, _)| *known == authority) {
        Some((_, runs)) => runs.push(team.run),
        None => held.push((authority, vec![team.run])),
      }
    }
    Ok(held)
  }
}

/// Whether the server sees `team`'s work running, as it last answered the run's reports: `work.start` made it running,
/// and only a report moves it on.
fn server_runs(team: &TeamRun) -> Result<bool, TeamWorkError> {
  let Some(accepted) = &team.accepted else {
    return Ok(true);
  };
  let work: WorkRead = serde_json::from_str(&accepted.work).map_err(|_| TeamWorkError::UnreadableRun(team.run))?;
  Ok(work.state == ExecutionState::Running)
}

fn refused(answer: Answer) -> Heartbeat {
  let error = match answer {
    Answer::Unreachable => return Heartbeat::Unreachable,
    Answer::Refused(error) => error,
  };
  match (error.code(), error.details()) {
    (AuthorityErrorCode::DecisionRequired, ErrorDetails::DecisionRequired { decisions }) => {
      Heartbeat::DecisionRequired(decisions.clone())
    }
    (AuthorityErrorCode::TransitionDenied, _) => Heartbeat::NotRunning,
    _ => Heartbeat::Refused(error),
  }
}
