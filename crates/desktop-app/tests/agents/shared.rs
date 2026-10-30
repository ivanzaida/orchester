//! A shared profile's page (`02d` `FuOXA`, `04` `w0Evp`, `06` `sakAV`): what it names that is missing here, a
//! teammate's version that waits for *Use v<N> here*, which synthetic input never presses, and *Runs here use*.
//! `cargo test -p orchester-app --test agents shared_profile`.

use std::collections::HashMap;

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  blob::ContentHash,
  connection::mcp::McpServerSelection,
  id::{ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  resource::{AgentProfile, VersionNumber},
  sharing::SharedPerson,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{
  Bound, BoundBy, BoundReference, CachedVersion, ProjectDb, ReferenceBinding, ReferenceTarget, SharingLink,
  VersionReference,
};

use super::harness::Agents;

/// Links the project and caches a teammate's v2 of profile `id`, which adds their MCP server `docs` (not here) and
/// names their connection `Work`, bound here by role to the project's Codex connection; v2 waits for the person.
fn shared_v2(agents: &Agents, id: ResourceId) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let v1 = db.latest_version::<AgentProfile>(id).expect("read").expect("v1");
  let local = v1.body().connection();
  let (theirs, docs) = (ConnectionId::generate(), ConnectionId::generate());
  let v2 = v1
    .body()
    .clone()
    .with_mcp_servers(McpServerSelection::new(vec![docs]).expect("servers"));
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
  let references = vec![
    VersionReference {
      target: ReferenceTarget::Connection(theirs),
      name: "Work".to_owned(),
      role: Some("codex_cli".to_owned()),
    },
    VersionReference {
      target: ReferenceTarget::McpServer(docs),
      name: "docs".to_owned(),
      role: None,
    },
  ];
  db.cache_shared_version(
    id,
    &CachedVersion {
      number: VersionNumber::FIRST.next().expect("v2"),
      published_at: Timestamp::now(),
      publisher: SharedPerson {
        id: UserId::generate(),
        name: "olha".to_owned(),
      },
      server_body: serde_json::to_value(&v2).expect("a body"),
      hash: ContentHash::new(&"b".repeat(64)).expect("a hash"),
      references,
      local_body: v2,
      waits_here: true,
    },
  )
  .expect("cached");
  for (reference, name, role) in [
    (
      BoundReference::Connection {
        reference: theirs,
        bound: Some(Bound {
          local,
          by: BoundBy::Role,
        }),
      },
      "Work",
      Some("codex_cli"),
    ),
    (
      BoundReference::McpServer {
        reference: docs,
        bound: None,
      },
      "docs",
      None,
    ),
  ] {
    db.set_reference_binding(&ReferenceBinding {
      reference,
      name: name.to_owned(),
      role: role.map(str::to_owned),
      changed_at: Timestamp::now(),
    })
    .expect("kept");
  }
}

#[test]
fn shared_profile_says_what_is_missing_and_only_a_person_uses_the_waiting_version_here() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  shared_v2(&agents, id);
  agents.open(id);
  agents.until("the shared bars", |s| s.exists("agent-widens-bar"));
  assert_eq!(
    agents.text("agent-missing-bar-status"),
    "Missing on this computer: MCP server “docs”"
  );
  assert!(agents.exists("agent-missing-choose"));
  assert_eq!(
    agents.text("agent-widens-bar-status"),
    "v2 widens what agents may do here"
  );
  assert!(
    agents
      .text("agent-widens-adds")
      .contains("An MCP server it did not give its agents"),
    "what v2 adds"
  );
  assert!(
    agents.exists("agent-runs-here-row"),
    "Runs here use, the person's own connection"
  );
  assert_eq!(agents.text("agent-runs-here"), "Test Codex");

  with_synthetic_input(|| agents.click("agent-use-here"));
  agents.until("the refusal", |s| s.exists("agent-use-here-person-only"));
  let db = || ProjectDb::open(&agents.root).expect("database");
  assert_eq!(
    db().waiting_version(id).expect("read").map(VersionNumber::get),
    Some(2),
    "synthetic input uses nothing"
  );

  agents.click("agent-use-here");
  agents.until("v2 used here", |s| !s.exists("agent-widens-bar"));
  assert_eq!(
    ProjectDb::open(&agents.root)
      .expect("database")
      .waiting_version(id)
      .expect("read"),
    None
  );
}
