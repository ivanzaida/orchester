//! A shared profile's page (`02d` `FuOXA`): a teammate's version that waits for *Use v<N> here*, which synthetic
//! input never presses, and the head that says which version runs use meanwhile.
//! `cargo test -p orchester-app --test agents shared_profile`.

use std::collections::HashMap;

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ProjectId, ResourceId, ServerId, UserId},
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, TeamCounterparts, VersionNumber},
  sharing::{SharedPerson, TaskRevision},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing, SharingLink};

use super::harness::Agents;

/// Links the project as a member and caches the team's v1 of profile `id`, and their v2, which adds a team MCP server;
/// v2 waits for the person, so runs use v1.
fn shared_v2(agents: &Agents, id: ResourceId) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let v1 = db.latest_version::<AgentProfile>(id).expect("read").expect("v1");
  let local = v1.body().connection();
  let connections = db.connections().expect("connections");
  let team_v1 = TeamAgentProfile::from_local(
    v1.body(),
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &[],
      team_mcp_servers: &[],
    },
  )
  .expect("the team's form");
  let docs = ResourceId::generate();
  let team_v2 = team_v1.clone().with_mcp_servers(vec![docs]).expect("servers");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: db.project().key().clone(),
    role: None,
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("linked");
  db.set_resource_sharing(id, Some(ResourceSharing::Shared(TaskRevision::FIRST)))
    .expect("the team's profile");
  let cli = ReferenceTarget::CliKind(AgentProvider::CodexCli);
  db.cache_shared_version(
    id,
    &cached(
      VersionNumber::FIRST,
      (&team_v1, AgentProfile::from_team(&team_v1, local)),
      vec![cli.clone()],
      false,
    ),
  )
  .expect("v1 cached");
  let server = ReferenceTarget::TeamResource {
    kind: ResourceKind::McpServer,
    id: docs,
  };
  db.cache_shared_version(
    id,
    &cached(
      VersionNumber::FIRST.next().expect("v2"),
      (&team_v2, AgentProfile::from_team(&team_v2, local)),
      vec![cli, server],
      true,
    ),
  )
  .expect("v2 cached");
}

/// Version `number` of a profile as the server confirmed it, published by the teammate olha.
fn cached(
  number: VersionNumber,
  (team, local): (&TeamAgentProfile, AgentProfile),
  references: Vec<ReferenceTarget>,
  waits_here: bool,
) -> CachedVersion<AgentProfile> {
  CachedVersion {
    number,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(team).expect("a body"),
    hash: ContentHash::new(&number.get().to_string().repeat(64)).expect("a hash"),
    references,
    local_body: local,
    waits_here,
  }
}

#[test]
fn shared_profile_only_a_person_uses_the_waiting_version_here() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  shared_v2(&agents, id);
  agents.open(id);
  agents.until("the widens bar", |s| s.exists("agent-widens-bar"));
  assert_eq!(
    agents.text("agent-widens-bar-status"),
    "v2 widens what agents may do here"
  );
  assert_eq!(
    agents.text("agent-widens-add-0"),
    "An MCP server it did not give its agents",
    "what v2 adds"
  );
  assert_eq!(
    agents.text("agent-team-state-state"),
    "Runs here use v1",
    "v2 waits, so runs keep v1 (02d)"
  );

  with_synthetic_input(|| agents.click("agent-use-here"));
  agents.until("the refusal", |s| s.exists("agent-use-here-person-only"));
  let root = agents.root.clone();
  let db = || ProjectDb::open(&root).expect("database");
  assert_eq!(
    db().waiting_version(id).expect("read").map(VersionNumber::get),
    Some(2),
    "synthetic input uses nothing"
  );

  agents.click("agent-use-here");
  agents.until("v2 used here", |s| !s.exists("agent-widens-bar"));
  assert_eq!(db().waiting_version(id).expect("read"), None);
  assert!(
    !agents.exists("agent-team-state-state") || agents.text("agent-team-state-state") != "Runs here use v1",
    "nothing waits now"
  );
}

/// Caches a teammate's v3 of profile `id`, the same as v2, which waits too.
fn shared_v3(agents: &Agents, id: ResourceId) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let v2 = db
    .cached_version::<AgentProfile>(id, VersionNumber::FIRST.next().expect("v2"))
    .expect("read")
    .expect("v2");
  db.cache_shared_version(
    id,
    &CachedVersion {
      number: v2.number.next().expect("v3"),
      hash: ContentHash::new(&"c".repeat(64)).expect("a hash"),
      ..v2
    },
  )
  .expect("cached");
}

#[test]
fn shared_profile_use_here_uses_the_version_the_bar_shows() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  shared_v2(&agents, id);
  agents.open(id);
  agents.until("v2 waits", |s| s.exists("agent-widens-bar"));
  assert_eq!(agents.text("agent-use-here"), "Use v2 here");

  // v3 arrives while the bar shows v2: pressing uses the version the bar showed.
  shared_v3(&agents, id);
  agents.click("agent-use-here");
  agents.until("v2 used here", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .latest_version::<AgentProfile>(id)
      .expect("read")
      .is_some_and(|version| version.number().get() == 2)
  });
  let db = ProjectDb::open(&agents.root).expect("database");
  assert_eq!(
    db.waiting_version(id).expect("read").map(VersionNumber::get),
    Some(3),
    "v3 still waits for its own press"
  );
}
