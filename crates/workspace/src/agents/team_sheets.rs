//! What the sheets of the team's tools, instructions and skills share: who published the version this computer uses,
//! and which of the team's profiles use a resource (`Agents · Tools` `u9Hcc` A6, `Agents · Instructions` `c3jU1R` A4,
//! `Agents · Skills` `c9YnL` A5).

use orchester_core::{
  id::ResourceId,
  resource::{ResourceBody, VersionNumber},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{team_mcp::Published, team_profile::TeamProfileSheet};

/// Who published version `number` of the resource `id` and when; `None` while the server has not confirmed it.
pub(super) fn published<B: ResourceBody>(
  db: &ProjectDb,
  id: ResourceId,
  number: VersionNumber,
) -> Result<Option<Published>, StoreError> {
  Ok(db.cached_version::<B>(id, number)?.map(|cached| Published {
    by: cached.publisher.name,
    at: cached.published_at,
  }))
}

/// The names of the team's profiles whose sheet `uses` the resource.
pub(super) fn used_by(
  db: &ProjectDb,
  profiles: &[(ResourceId, TeamProfileSheet)],
  uses: impl Fn(&TeamProfileSheet) -> bool,
) -> Result<Vec<String>, StoreError> {
  let mut names = Vec::new();
  for (profile, sheet) in profiles {
    if uses(sheet)
      && let Some(found) = db.resource(*profile)?
    {
      names.push(found.name().to_string());
    }
  }
  Ok(names)
}
