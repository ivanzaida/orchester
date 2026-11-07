//! The team's resources as this computer has them cached, so `run_task` can read whether the pipeline is ready here
//! before it asks the team server for anything.

use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ResourceId, UserId},
  name::Name,
  resource::{AgentProfile, Resource, ResourceKind, Slug, TeamAgentProfile, VersionNumber},
  sharing::{SharedPerson, TaskRevision},
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb};
use serde_json::{Value, json};

use crate::{
  fixture::{self, Config},
  pipelines::team_pipeline_running,
};

/// Caches the team's pipeline and the Builder profile of `config` (which runs on `cli`, in the team's form) as the team
/// server confirmed them, so the pipeline is ready on a computer with a connection of `cli`.
pub(crate) fn team_ready(db: &mut ProjectDb, config: &Config, cli: AgentProvider) {
  team_pipeline_running(db, config.pipeline, "delivery", config.profile);
  let local = fixture::builder();
  let body = local.body();
  let team = TeamAgentProfile::new(
    body.role().clone(),
    body.responsibilities().to_owned(),
    body.instructions().clone(),
    (cli, body.model().clone(), body.settings().clone()),
  );
  cache_profile(db, config.profile, &team, json!(team));
}

/// Keeps `id` as the team's profile `builder` with `server_body` as the server's version 1, which this computer runs by
/// the local body made of `team`.
pub(crate) fn cache_profile(db: &mut ProjectDb, id: ResourceId, team: &TeamAgentProfile, server_body: Value) {
  keep(db, id, ResourceKind::AgentProfile, "builder");
  let local = AgentProfile::from_team(team, AgentProfile::unresolved_connection(id));
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Dana".to_owned(),
    },
    server_body,
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body: local,
    waits_here: false,
  };
  db.cache_shared_version(id, &version).expect("the profile's version");
}

fn keep(db: &mut ProjectDb, id: ResourceId, kind: ResourceKind, slug: &str) {
  let resource = Resource::new(
    id,
    kind,
    Slug::new(slug).expect("slug"),
    Name::new(slug).expect("name"),
    Timestamp::now(),
  );
  db.keep_shared_resource(&resource, TaskRevision::try_from(1).expect("a revision"))
    .expect("the team's resource");
}
