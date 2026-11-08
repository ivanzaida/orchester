//! A team task's newest start as the project's records tell it (`orchester_workspace::team_work::last_start`): the
//! page's notice for a start that waited and was settled later, by the recovery of the project's open starts, which no
//! window was asking about. Local to this computer, so a team server's change does not make it stale; it is read again
//! when the task is, and when recovery settles a start ([`super::invalidation::Stale`]).

use orchester_core::id::TaskId;
use orchester_workspace::{background, team_work};

use super::{
  source::{TeamFailure, TeamSource},
  start::{Recorded, recorded},
};

/// The newest start of the task `task`, `None` when this computer never started it.
#[lurq::query(stale_time = "30s", gc_time = "5m")]
pub async fn team_last_start(source: TeamSource, task: TaskId) -> Result<Option<Recorded>, TeamFailure> {
  let root = source.root().to_owned();
  let read =
    background::run(move || team_work::last_start(&root, task).map(|last| last.map(|last| recorded(last, &root))))
      .await;
  match read {
    Ok(Ok(last)) => Ok(last),
    Ok(Err(error)) => {
      tracing::warn!(%error, "the newest start of a team task cannot be read");
      Err(TeamFailure::Refused)
    }
    Err(_) => Err(TeamFailure::Refused),
  }
}
