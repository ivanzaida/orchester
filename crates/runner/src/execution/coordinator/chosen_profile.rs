//! The agent profile a person chose for a step they return a run's work to (`Run::executing_agent`): it must be a
//! version the run can read, and the model it names must be that version's, so what the history records is what runs.
//! A run that has no inputs reads any published version; a run with inputs (a team's) reads only the versions they
//! pin, and the bundle is never changed, so another profile is refused instead of being added to it.

use orchester_core::{
  id::RunId,
  run::{ChosenProfile, RunnerFailure},
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::link::LocalLink;
use crate::execution::{
  executor::resolver::{ResolveError, RunResolver},
  failed,
};

/// Refuses `chosen`, when there is one, as [`Rejection::ProfileNotUsable`] unless `run` can run it.
pub(super) fn check(db: &ProjectDb, run: RunId, chosen: Option<&ChosenProfile>) -> Result<(), Rejection> {
  let Some(chosen) = chosen else {
    return Ok(());
  };
  let resolver = RunResolver::load(db, run).map_err(|error| failed("cannot read the run's inputs", &error))?;
  match resolver.profile(&LocalLink::over(db), chosen.profile) {
    Ok(body) if body.model() == &chosen.model => Ok(()),
    Ok(_) | Err(ResolveError::Gone { .. } | ResolveError::VersionMissing { .. }) => Err(Rejection::ProfileNotUsable),
    // A resource the project does not have is not a storage failure: the link says so as any other failure.
    Err(ResolveError::Link(error)) if error.failure == RunnerFailure::Other => Err(Rejection::ProfileNotUsable),
    Err(error) => Err(failed("cannot read the chosen agent profile", &error)),
  }
}
