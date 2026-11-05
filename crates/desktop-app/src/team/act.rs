//! The team's actions from the screens (team authority P6c, `docs/design/team-authority.md` §2–5, §7): each goes to the
//! project's server through `orchester_workspace`'s dispatcher in a session of its own, made for the project's link,
//! never through a local role flag: the server's capabilities say what is offered, and its answer what came of it
//! ([`Acted`]). The dispatcher's futures hold the project database across their awaits, so they run on a worker of the
//! runtime. Once the server took an action, the session's reads of the project are read again at once; the
//! subscription says so too, a little later.

use orchester_api::v2::{AuthorityErrorCode, TeamCommandValue};
use orchester_core::id::ClaimId;
use orchester_workspace::{
  actions::{Accepted, Action, ActionError, ActionOutcome, Current, Dispatcher, Intent, Refusal, TeamOutcome},
  background,
  sharing::Servers,
};
use tokio::runtime::Handle;

use super::{TeamQueries, TeamSource, invalidation::Stale};

/// What became of a team action, as the screens say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Acted {
  /// The server took it.
  Done,
  /// The server refused it for good, with its code: nothing changed.
  Refused(AuthorityErrorCode),
  /// The subject changed on the server since it was read: nothing of the change was saved; what it holds now, when it
  /// could be read.
  Changed(Option<Holds>),
  /// The server cannot be reached or refused for now, or its answer was lost: nothing is known to have changed, and
  /// asking again is safe.
  Unavailable,
  /// This computer is not signed in to the server.
  SignedOut,
  /// It could not be carried out here.
  Failed,
}

impl lurq::app::component::DevtoolsInspectable for Acted {}

/// What a revision-checked subject holds now, after a change was refused for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Holds {
  /// A module's branch, and who changed the module last.
  Branch { branch: String, by: String },
  /// The workspace's automatic merge.
  AutomaticMerge(bool),
}

/// What the screens need to carry out a team action: the session's servers and team reads, and the runtime.
#[derive(Clone)]
pub struct Acting {
  pub servers: Servers,
  pub team: TeamQueries,
  pub runtime: Handle,
}

impl Acting {
  pub fn of(services: &crate::services::Services) -> Self {
    Self {
      servers: services.servers.clone(),
      team: services.team.clone(),
      runtime: services.runtime.clone(),
    }
  }

  /// The project's dispatcher in a session of its own; `None` while this computer is not signed in there.
  async fn dispatcher(&self, source: &TeamSource) -> Option<Dispatcher> {
    let key = source.key();
    match self.servers.team_session(key.server, key.project).await {
      Ok(session) => Some(Dispatcher::new(
        source.root().to_owned(),
        self.servers.clone(),
        Some(session),
      )),
      Err(error) => {
        tracing::warn!(%error, "no team session for a screen's action");
        None
      }
    }
  }

  /// Carries out `action` on `source`'s project; what the server made of it, with its receipt's claim when it gave one.
  pub async fn act(&self, source: &TeamSource, action: Action) -> (Acted, Option<ClaimId>) {
    self.act_intent(source, Intent::new(action)).await
  }

  /// [`Self::act`] of `intent`, whose operation the caller keeps, e.g. an authorization a request cites.
  pub async fn act_intent(&self, source: &TeamSource, intent: Intent) -> (Acted, Option<ClaimId>) {
    let (acting, source) = (self.clone(), source.clone());
    let runtime = self.runtime.clone();
    let answered = background::run(move || {
      runtime.block_on(async move {
        let Some(dispatcher) = acting.dispatcher(&source).await else {
          return (Acted::SignedOut, None);
        };
        acted(dispatcher.dispatch(&intent).await)
      })
    })
    .await;
    let answered = answered.unwrap_or((Acted::Failed, None));
    // The server answered: what it holds now is read again, the subject a refusal names too.
    if matches!(answered.0, Acted::Done | Acted::Refused(_) | Acted::Changed(_)) {
      Stale::EVERY.apply(&self.team.client());
    }
    answered
  }
}

/// What `outcome` came to, with the claim a claim command's receipt gives.
fn acted(outcome: Result<ActionOutcome, ActionError>) -> (Acted, Option<ClaimId>) {
  let outcome = match outcome {
    Ok(ActionOutcome::Team(outcome)) => outcome,
    // The project was found local after all: nothing of the team's was changed.
    Ok(ActionOutcome::Local(_)) => return (Acted::Failed, None),
    Err(ActionError::NoTeamSession | ActionError::SessionChanged | ActionError::OtherProject) => {
      return (Acted::SignedOut, None);
    }
    Err(error) => {
      tracing::warn!(%error, "a screen's team action failed");
      return (Acted::Failed, None);
    }
  };
  match outcome {
    TeamOutcome::Completed(Accepted::Command(receipt))
    | TeamOutcome::AcceptedPending { receipt, .. }
    | TeamOutcome::LocalEffectFailed { receipt, .. } => {
      let claim = match receipt.result {
        TeamCommandValue::Claim { claim, .. } => Some(claim.id),
        _ => None,
      };
      (Acted::Done, claim)
    }
    TeamOutcome::Completed(Accepted::Definition(_)) => (Acted::Done, None),
    TeamOutcome::Rejected { retryable: true, .. } | TeamOutcome::AcceptanceUnknown { .. } => (Acted::Unavailable, None),
    TeamOutcome::Rejected { refusal, current, .. } => (refused(&refusal, current), None),
  }
}

/// A final refusal: a revision conflict with what the subject holds now, else the server's code.
fn refused(refusal: &Refusal, current: Option<Current>) -> Acted {
  match refusal {
    Refusal::Command(error) if error.code() == AuthorityErrorCode::RevisionConflict => {
      Acted::Changed(current.and_then(holds))
    }
    Refusal::Command(error) => Acted::Refused(error.code()),
    Refusal::PolicyRevisionConflict | Refusal::Superseded => Acted::Changed(current.and_then(holds)),
    Refusal::Route { code, .. } if *code == orchester_api::ErrorCode::RevisionConflict => {
      Acted::Changed(current.and_then(holds))
    }
    Refusal::Route { .. } => Acted::Refused(AuthorityErrorCode::Forbidden),
  }
}

/// What a subject read after a conflict holds, where the screens show it.
fn holds(current: Current) -> Option<Holds> {
  match current {
    Current::ModuleRead(module) => Some(Holds::Branch {
      branch: module.branch,
      by: module.module.updated.by.display_name,
    }),
    Current::Policy(policy) => Some(Holds::AutomaticMerge(policy.automatic_merge)),
    Current::Module(_) | Current::Task(_) | Current::TaskRead(_) | Current::Tags(_) | Current::NotRead(_) => None,
  }
}
