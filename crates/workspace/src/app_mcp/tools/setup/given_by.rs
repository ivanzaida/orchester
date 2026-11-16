//! Which local profiles give their agents an instruction, skill or tool (`docs/design/team-configuration.md`, "Decided
//! 2026-11-09"): in a local project nothing else gives an agent one, so the tools that read these resources say it as
//! `used_by`, each profile `{id, name}` whose latest published version or draft lists it, by name.

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, ResourceKind},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::{Value, json};

/// The project's profiles that are not archived, each with the bodies of its latest version and draft.
pub(super) struct Givers(Vec<(ResourceId, String, Vec<AgentProfile>)>);

impl Givers {
  pub(super) fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    let mut profiles = Vec::new();
    for resource in db.resources(ResourceKind::AgentProfile)? {
      let mut bodies = Vec::new();
      if let Some(version) = db.latest_version::<AgentProfile>(resource.id())? {
        bodies.push(version.body().clone());
      }
      if let Some(draft) = db.draft::<AgentProfile>(resource.id())? {
        bodies.push(draft.body().clone());
      }
      profiles.push((resource.id(), resource.name().to_string(), bodies));
    }
    profiles.sort_by_cached_key(|(id, name, _)| (name.to_lowercase(), id.to_string()));
    Ok(Self(profiles))
  }

  /// The profiles that give `resource`, as `used_by` lists them.
  pub(super) fn of(&self, resource: ResourceId) -> Value {
    self
      .0
      .iter()
      .filter(|(_, _, bodies)| bodies.iter().any(|body| body.gives(resource)))
      .map(|(id, name, _)| json!({ "id": id.to_string(), "name": name }))
      .collect()
  }
}
