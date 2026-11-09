//! A profile kept as the team's in a test project database.

use orchester_core::{
  blob::ContentHash,
  id::{ResourceId, UserId},
  resource::{AgentProfile, ResourceKind},
  sharing::{SharedPerson, TaskRevision},
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing};

use super::at;

/// Makes `profile`, published as `version`, the team's: its version is cached as the server confirmed it, naming
/// `servers` by `TeamResource` references.
pub(super) fn keep_as_team_profile(
  db: &mut ProjectDb,
  profile: ResourceId,
  version: &orchester_core::resource::Version<AgentProfile>,
  servers: &[ResourceId],
) {
  let revision = TaskRevision::try_from(1_u64).expect("a revision");
  db.set_resource_sharing(profile, Some(ResourceSharing::Shared(revision)))
    .expect("the profile is the team's");
  let references = team_server_references(servers);
  let confirmed = CachedVersion {
    number: version.number(),
    published_at: at(3),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: serde_json::json!({}),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references,
    local_body: version.body().clone(),
    waits_here: false,
  };
  db.cache_shared_version(profile, &confirmed)
    .expect("the team version is cached");
}

/// The references of a team profile version that chooses the team MCP servers `servers`.
pub(super) fn team_server_references(servers: &[ResourceId]) -> Vec<ReferenceTarget> {
  servers
    .iter()
    .map(|&id| ReferenceTarget::TeamResource {
      kind: ResourceKind::McpServer,
      id,
    })
    .collect()
}
