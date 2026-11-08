//! The CLI kind a profile that uses a resource runs, for the mark a list draws at the profile's row.

use std::path::Path;

use orchester_core::{connection::AgentProvider, id::ResourceId, resource::AgentProfile, store::ResourceStore};
use orchester_store::{ProjectDb, StoreError};

use crate::readiness::team_profile_body;

/// The CLI kind of the team profile `profile`, as its latest version in the project at `root` says; `None` for a
/// profile this computer has no version of or whose version is not in the team's form, which names no CLI. Blocks on
/// the project database.
pub fn profile_cli(root: &Path, profile: ResourceId) -> Result<Option<AgentProvider>, StoreError> {
  let db = ProjectDb::open(root)?;
  let Some(version) = db.latest_version::<AgentProfile>(profile)? else {
    return Ok(None);
  };
  let waiting = db.waiting_publishes()?;
  // A version that is not in the team's form is `Undecided` for readiness; here it only has no mark to draw.
  Ok(
    team_profile_body(&db, profile, &version, &waiting)?
      .ok()
      .map(|team| team.cli()),
  )
}
