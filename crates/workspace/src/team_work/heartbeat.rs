//! The exact heartbeat of team work (S5, OA-31): this computer keeps the claim of a team task for as long as it holds
//! work of it, and the claim of a module for as long as its delivery goes on here. Each claim is named once, by its
//! exact authority with no work (the server's `docs/claim-authority.md`: "`work_id` must be `null`").
//!
//! - **Running work.** The run at work, and also one whose step waits for a person (an approval, an answer, a plan),
//!   one stuck for a person's decision, one a person paused and one interrupted here: none of those ends the server's
//!   work, which only a report does.
//! - **Kept work.** Work the server saw succeed, while its run is not merged here or the Done its merge asks for is not
//!   answered yet (OA-30, which releases the claim), and work paused at a limit, which the run may still carry on.
//! - **Module claims.** The claim the newest delivery of a module was requested under, while that delivery goes on.
//!
//! A team server older than this rule renews running work only and refuses the whole heartbeat for kept work or a
//! module claim (`decision_required` or `transition_denied`); the heartbeat then names the running claims alone, so
//! they are renewed as before. `decision_required` is an outcome to show, never a failure to retry; an authority the
//! server lists as lost fences its runs, which are then stopped ([`super::fenced`]).

use orchester_api::v2::{
  AuthorityError, AuthorityErrorCode, AuthorityRef, DecisionId, ErrorDetails, ExactHeartbeat, ExecutionState,
  MAX_HEARTBEAT_AUTHORITIES, Subject, WorkRead,
};
use orchester_core::{id::RunId, store::MergeStore, time::Timestamp};
use orchester_store::{MergedDoneState, ProjectDb, TeamFence, TeamRun};

use super::{Answer, TeamWork, TeamWorkError, answer, delivery::held_module_claims};

/// What one heartbeat did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heartbeat {
  /// No team run or delivery here holds work the server keeps under a claim: nothing was sent.
  NothingToRenew,
  /// The claims of `renewed` runs are renewed; those of `lost` runs are gone, and the runs are fenced. A module claim
  /// renewed or lost names no run.
  Renewed { renewed: Vec<RunId>, lost: Vec<RunId> },
  /// The server renews nothing while the work of a named claim waits for a person: the decisions it names are open (D1
  /// for a task's).
  DecisionRequired(Vec<DecisionId>),
  /// The server renews nothing: a named claim's work is not running there.
  NotRunning,
  /// The server cannot be reached, or refused for now; the next heartbeat tries again.
  Unreachable,
  /// The server refused the heartbeat for another reason, kept as it came.
  Refused(AuthorityError),
}

/// A claim to renew and the runs here that hold work under it (none for a module's).
type Held = (AuthorityRef, Vec<RunId>);

/// The claims one heartbeat names.
struct Named {
  /// Of work the server sees running: renewed by every team server.
  running: Vec<Held>,
  /// Of kept work and module claims: renewed by a server that keeps them (OA-31).
  kept: Vec<Held>,
}

/// How a team run's work stands for the heartbeat.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Holds {
  Running,
  Kept,
  Nothing,
}

impl<R> TeamWork<R> {
  /// Renews the claims of the team work this computer holds, at most [`MAX_HEARTBEAT_AUTHORITIES`] at a time.
  pub async fn heartbeat(&self) -> Result<Heartbeat, TeamWorkError> {
    let named = self.named()?;
    if named.running.is_empty() && named.kept.is_empty() {
      return Ok(Heartbeat::NothingToRenew);
    }
    let all: Vec<Held> = named.running.iter().chain(&named.kept).cloned().collect();
    match self.send_heartbeat(&all).await? {
      Err(Answer::Refused(error)) if !named.kept.is_empty() && older_rule(&error) => {
        tracing::info!(code = ?error.code(), "the team server renews running work only: its running claims are named alone");
        if named.running.is_empty() {
          return Ok(refused(Answer::Refused(error)));
        }
        Ok(self.send_heartbeat(&named.running).await?.unwrap_or_else(refused))
      }
      sent => Ok(sent.unwrap_or_else(refused)),
    }
  }

  /// Sends `held` in chunks: the runs renewed and lost, or the first answer that renewed nothing.
  async fn send_heartbeat(&self, held: &[Held]) -> Result<Result<Heartbeat, Answer>, TeamWorkError> {
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    let (mut renewed, mut lost) = (Vec::new(), Vec::new());
    for chunk in held.chunks(MAX_HEARTBEAT_AUTHORITIES) {
      let heartbeat = ExactHeartbeat {
        authorities: chunk.iter().map(|(authority, _)| *authority).collect(),
      };
      let answered = match session.authority().heartbeat(&heartbeat).await {
        Ok(answered) => answered,
        Err(error) => return Ok(Err(answer(error))),
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
    Ok(Ok(Heartbeat::Renewed { renewed, lost }))
  }

  /// The claims of the unfenced team runs that hold work, each once with the runs under it, and the module claims of
  /// the deliveries that go on. A claim with work running is running; one whose newest run holds kept work is kept.
  fn named(&self) -> Result<Named, TeamWorkError> {
    let db = self.db()?;
    let mut claims: Vec<(AuthorityRef, Vec<(RunId, Holds)>)> = Vec::new();
    for team in db.team_runs()? {
      if team.fence.is_some() {
        continue;
      }
      let authority = AuthorityRef {
        subject: Subject::Task { id: team.team_task },
        claim_id: team.claim,
        work_id: None,
      };
      let holds = holds(&db, &team)?;
      match claims.iter_mut().find(|(known, _)| *known == authority) {
        Some((_, runs)) => runs.push((team.run, holds)),
        None => claims.push((authority, vec![(team.run, holds)])),
      }
    }
    let mut named = Named {
      running: Vec::new(),
      kept: Vec::new(),
    };
    for (authority, runs) in claims {
      let held = |wanted: &[Holds]| -> Vec<RunId> {
        runs
          .iter()
          .filter(|(_, holds)| wanted.contains(holds))
          .map(|(run, _)| *run)
          .collect()
      };
      if runs.iter().any(|(_, holds)| *holds == Holds::Running) {
        named.running.push((authority, held(&[Holds::Running, Holds::Kept])));
      } else if runs.last().is_some_and(|(_, holds)| *holds == Holds::Kept) {
        named.kept.push((authority, held(&[Holds::Kept])));
      }
    }
    for (module, claim_id) in held_module_claims(&db)? {
      let authority = AuthorityRef {
        subject: Subject::Module { id: module },
        claim_id,
        work_id: None,
      };
      named.kept.push((authority, Vec::new()));
    }
    Ok(named)
  }
}

/// How `team`'s work stands, as the server last answered the run's reports: `work.start` made it running, and only a
/// report moves it on. Succeeded work is kept until the run is merged here and the server answered the Done its merge
/// asks for; paused work (at a limit) is kept.
fn holds(db: &ProjectDb, team: &TeamRun) -> Result<Holds, TeamWorkError> {
  let Some(accepted) = &team.accepted else {
    return Ok(Holds::Running);
  };
  let work: WorkRead = serde_json::from_str(&accepted.work).map_err(|_| TeamWorkError::UnreadableRun(team.run))?;
  Ok(match work.state {
    ExecutionState::Running => Holds::Running,
    ExecutionState::Paused => Holds::Kept,
    ExecutionState::Succeeded if db.run_merge(team.run)?.is_none() => Holds::Kept,
    ExecutionState::Succeeded => match db.merged_done(team.run)? {
      Some(done) if done.state == MergedDoneState::Pending => Holds::Kept,
      _ => Holds::Nothing,
    },
    ExecutionState::AwaitingHuman | ExecutionState::Failed | ExecutionState::Cancelled => Holds::Nothing,
  })
}

/// Whether `error` is how a team server older than OA-31's renewal refuses kept work or a module claim.
fn older_rule(error: &AuthorityError) -> bool {
  matches!(
    error.code(),
    AuthorityErrorCode::DecisionRequired | AuthorityErrorCode::TransitionDenied
  )
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
