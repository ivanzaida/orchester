//! The agent profile a run's step runs on, at the version the run pinned. A local project's database keeps every
//! version of its profiles. A team project's keeps no copy of the team's (`docs/design/team-configuration-live.md`):
//! a team run carries the profiles it pinned in its own inputs, the bundle the team server accepted with the
//! connection this computer bound each to ([`orchester_core::run::RunInputs::profile`]), so they are read from there.

use orchester_core::{id::RunId, resource::AgentProfile, run::PinnedVersion, store::ResourceStore};
use orchester_store::{ProjectDb, StoreError};

/// The profile version `pinned` that a step of run `run` runs on: the one the project's database keeps, else the one
/// the run's inputs hold. `None` when neither has it (a version removed since, in a run created without inputs).
pub(crate) fn pinned_profile(
  db: &ProjectDb,
  run: RunId,
  pinned: PinnedVersion,
) -> Result<Option<AgentProfile>, StoreError> {
  if let Some(version) = db.version::<AgentProfile>(pinned.resource, pinned.number)? {
    return Ok(Some(version.body().clone()));
  }
  Ok(db.run_inputs(run)?.and_then(|inputs| inputs.profile(pinned)))
}
