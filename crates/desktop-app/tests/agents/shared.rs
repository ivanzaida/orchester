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

/// Links the project as a member and caches the team's v1 of profile `id`, which names the teammate's connection
/// `Work`, bound here by role to the project's Codex connection, and their v2, which adds their MCP server `docs` (not
/// here); v2 waits for the person, so runs use v1.
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
  let work = VersionReference {
    target: ReferenceTarget::Connection(theirs),
    name: "Work".to_owned(),
    role: Some("codex_cli".to_owned()),
  };
  db.cache_shared_version(id, &cached(VersionNumber::FIRST, v1.body(), vec![work.clone()], false))
    .expect("v1 cached");
  let docs_reference = VersionReference {
    target: ReferenceTarget::McpServer(docs),
    name: "docs".to_owned(),
    role: None,
  };
  db.cache_shared_version(
    id,
    &cached(
      VersionNumber::FIRST.next().expect("v2"),
      &v2,
      vec![work, docs_reference],
      true,
    ),
  )
  .expect("v2 cached");
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

/// Version `number` of a profile as the server confirmed it, published by the teammate olha.
fn cached(
  number: VersionNumber,
  body: &AgentProfile,
  references: Vec<VersionReference>,
  waits_here: bool,
) -> CachedVersion<AgentProfile> {
  CachedVersion {
    number,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(body).expect("a body"),
    hash: ContentHash::new(&number.get().to_string().repeat(64)).expect("a hash"),
    references,
    local_body: body.clone(),
    waits_here,
  }
}

#[test]
fn shared_profile_says_what_is_missing_and_only_a_person_uses_the_waiting_version_here() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  shared_v2(&agents, id);
  agents.open(id);
  agents.until("the shared bars", |s| s.exists("agent-widens-bar"));
  assert!(
    !agents.exists("agent-missing-bar"),
    "runs use v1, which names nothing missing here"
  );
  assert_eq!(
    agents.text("agent-widens-bar-status"),
    "v2 widens what agents may do here"
  );
  assert_eq!(
    agents.text("agent-widens-add-0"),
    "MCP server docs · missing on this computer",
    "what v2 adds, by name"
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
  agents.until("v2 used here", |s| {
    !s.exists("agent-widens-bar") && s.exists("agent-missing-bar-status")
  });
  assert_eq!(
    ProjectDb::open(&agents.root)
      .expect("database")
      .waiting_version(id)
      .expect("read"),
    None
  );
  assert_eq!(
    agents.text("agent-missing-bar-status"),
    "Missing on this computer: MCP server “docs”",
    "runs use v2 now, which names docs"
  );
  assert!(agents.exists("agent-missing-choose"));
  assert_eq!(
    agents.text("agent-team-state-runs-here"),
    "runs here: Test Codex",
    "nothing waits now: the head says which connection runs take here (06)"
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

/// Opens *Runs here use*'s menu, scrolled into view as a person sees it first.
fn open_runs_here(agents: &mut Agents) {
  agents
    .window
    .tree
    .get_element_by_id_mut("agent-runs-here")
    .expect("Runs here use")
    .scroll_into_view();
  agents.window.pass();
  agents.click("agent-runs-here");
  agents.until("the picker's menu", |s| s.exists("agent-runs-here-option-0"));
}
#[test]
fn shared_profile_head_says_where_it_runs_and_synthetic_input_chooses_no_connection() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  shared_v2(&agents, id);
  agents.open(id);
  agents.until("the shared parts", |s| s.exists("agent-cli-value"));
  assert_eq!(
    agents.text("agent-team-state-state"),
    "Runs here use v1",
    "v2 waits, so runs keep v1 (02d)"
  );
  // The team chose the CLI, the model and the effort: a member reads them (06).
  assert_eq!(agents.text("agent-cli-value"), "Codex");
  assert_eq!(
    agents.text("agent-cli-hint"),
    "The team's choice in Builder v1 · admins change it"
  );
  assert_eq!(agents.text("agent-effort-hint"), "The team's choice");
  assert!(
    !agents.exists("agent-validation-error"),
    "nothing is edited on a member's page, so nothing is invalid there"
  );

  open_runs_here(&mut agents);
  with_synthetic_input(|| agents.click("agent-runs-here-option-0"));
  agents.until("the refusal", |s| s.exists("agent-runs-here-person-only"));
  let bindings = ProjectDb::open(&agents.root)
    .expect("database")
    .reference_bindings()
    .expect("bindings");
  let work = bindings.iter().find(|binding| binding.name == "Work").expect("Work");
  assert!(
    matches!(
      work.reference,
      BoundReference::Connection {
        bound: Some(Bound { by: BoundBy::Role, .. }),
        ..
      }
    ),
    "synthetic input chose nothing: {work:?}"
  );
}

#[test]
fn shared_profile_use_here_uses_the_version_the_bar_shows() {
  let mut agents = Agents::new(true);
  let id = agents.profile(true);
  shared_v2(&agents, id);
  agents.open(id);
  agents.until("v2 waits", |s| s.exists("agent-widens-bar"));
  assert_eq!(
    agents.text("agent-widens-bar-status"),
    "v2 widens what agents may do here"
  );

  // v3 arrives; choosing the connection here reads the page again.
  shared_v3(&agents, id);
  open_runs_here(&mut agents);
  agents.click("agent-runs-here-option-0");
  agents.until("v3 waits", |s| {
    s.exists("agent-widens-bar-status") && s.text("agent-widens-bar-status") == "v3 widens what agents may do here"
  });
  assert_eq!(agents.text("agent-use-here"), "Use v3 here");

  agents.press_until("agent-use-here", "v3 used here", |s| !s.exists("agent-widens-bar"));
  let db = ProjectDb::open(&agents.root).expect("database");
  assert_eq!(db.waiting_version(id).expect("read"), None, "nothing waits");
  assert_eq!(
    db.latest_version::<AgentProfile>(id)
      .expect("read")
      .map(|version| version.number().get()),
    Some(3),
    "runs use v3, the version the bar showed"
  );
}
