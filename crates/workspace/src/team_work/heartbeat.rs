//! The exact heartbeat of team runs (S5, D1): the claims of runs at work here are renewed by their exact authority, and
//! only theirs. A run that waits for a person, is paused, interrupted or ended renews nothing, and when no run works
//! nothing is sent. The server's `decision_required` for a claim whose work waits is an outcome to show, never a
//! failure to retry; an authority it lists as lost fences its run.

use orchester_api::v2::{
  AuthorityError, AuthorityErrorCode, AuthorityRef, DecisionId, ErrorDetails, ExactHeartbeat,
  MAX_HEARTBEAT_AUTHORITIES, Subject,
};
use orchester_core::{id::RunId, run::renews_claim, store::RunStore, time::Timestamp};
use orchester_store::{StoreError, TeamFence};

use super::{Answer, TeamRunner, TeamWork, TeamWorkError, answer};

/// What one heartbeat did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heartbeat {
  /// No team run works here now: nothing was sent, and no claim is renewed.
  NothingRunning,
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

impl<R: TeamRunner> TeamWork<R> {
  /// Renews the claims of the team runs at work here, at most [`MAX_HEARTBEAT_AUTHORITIES`] at a time.
  pub async fn heartbeat(&self) -> Result<Heartbeat, TeamWorkError> {
    let running = self.running()?;
    if running.is_empty() {
      return Ok(Heartbeat::NothingRunning);
    }
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    let (mut renewed, mut lost) = (Vec::new(), Vec::new());
    for chunk in running.chunks(MAX_HEARTBEAT_AUTHORITIES) {
      let heartbeat = ExactHeartbeat {
        authorities: chunk.iter().map(|(_, authority)| *authority).collect(),
      };
      let answered = match session.authority().heartbeat(&heartbeat).await {
        Ok(answered) => answered,
        Err(error) => return Ok(refused(answer(error))),
      };
      for (run, authority) in chunk {
        if answered.lost.contains(authority) {
          self
            .db()?
            .fence_team_run(*run, TeamFence::ClaimLost, Timestamp::now())?;
          lost.push(*run);
        } else {
          renewed.push(*run);
        }
      }
    }
    Ok(Heartbeat::Renewed { renewed, lost })
  }

  /// The unfenced team runs at work here (core's [`renews_claim`]) with the authority each reports under.
  fn running(&self) -> Result<Vec<(RunId, AuthorityRef)>, TeamWorkError> {
    let db = self.db()?;
    let mut running = Vec::new();
    for team in db.team_runs()? {
      if team.fence.is_some() {
        continue;
      }
      let run = db.run(team.run)?.ok_or(StoreError::RunNotFound(team.run))?;
      if renews_claim(&run) {
        let authority = AuthorityRef {
          subject: Subject::Task { id: team.team_task },
          claim_id: team.claim,
          work_id: Some(team.work),
        };
        running.push((team.run, authority));
      }
    }
    Ok(running)
  }
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
