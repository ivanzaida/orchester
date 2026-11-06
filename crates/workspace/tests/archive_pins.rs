//! The runs a team MCP server's archive stops (`docs/architecture/server.md`, row 79): a run pins no MCP server
//! itself, so it is one that has not ended and pins a profile version choosing the server; a run that pins a profile
//! choosing other servers, or none, is left alone.
//!
//! `cargo test -p orchester-workspace --test archive_pins archive_pins_`

mod support;

use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{Resource, ResourceKind, Slug},
  sharing::TaskRevision,
};
use orchester_workspace::sharing::sync::pinning;
use support::{Fixture, at};

/// The team MCP server `id` named `called`, kept in the fixture's project as a synced one is.
fn keep_team_server(fixture: &mut Fixture, id: ResourceId, called: &str) {
  let server = Resource::new(
    id,
    ResourceKind::McpServer,
    Slug::new(&called.to_lowercase()).expect("slug"),
    Name::new(called).expect("name"),
    at(1),
  );
  let revision = TaskRevision::try_from(1_u64).expect("a revision");
  fixture
    .db
    .keep_shared_resource(&server, revision)
    .expect("the team MCP server is kept");
}

/// How a profile's selection names the team MCP server `server`.
fn chosen(server: ResourceId) -> ConnectionId {
  ConnectionId::from_uuid(server.as_uuid())
}

#[test]
fn archive_pins_a_run_whose_profile_chooses_the_team_mcp_server_is_stopped_by_its_archive() {
  let (knowledge, other) = (ResourceId::generate(), ResourceId::generate());
  let mut fixture = Fixture::choosing(&[chosen(knowledge)]);
  keep_team_server(&mut fixture, knowledge, "Knowledge");
  keep_team_server(&mut fixture, other, "Other");
  let task = fixture.task("Search", at(10));
  let run = fixture.started_run(task, at(11));

  // The archive delta marks the resource archived before its runs are read (`apply.rs`).
  for archived in [knowledge, other] {
    fixture
      .db
      .mark_shared_archived(archived, true, at(12))
      .expect("the archive is marked");
  }

  let root = fixture.repo.path();
  let stopped = pinning(root, knowledge).expect("read").expect("the server is kept");
  assert_eq!(stopped.runs(), [(run.id(), task)]);
  assert_eq!(stopped.resource().as_str(), "Knowledge");
  let spared = pinning(root, other).expect("read").expect("the server is kept");
  assert!(
    spared.runs().is_empty(),
    "a server no pinned profile chooses stops nothing"
  );
  assert!(
    pinning(root, ResourceId::generate()).expect("read").is_none(),
    "a resource this project does not keep stops nothing"
  );
}

#[test]
fn archive_pins_a_run_whose_profile_chooses_no_team_mcp_server_is_not_stopped() {
  let knowledge = ResourceId::generate();
  let mut fixture = Fixture::choosing(&[]);
  keep_team_server(&mut fixture, knowledge, "Knowledge");
  let task = fixture.task("Search", at(10));
  fixture.started_run(task, at(11));

  fixture
    .db
    .mark_shared_archived(knowledge, true, at(12))
    .expect("the archive is marked");
  let stopped = pinning(fixture.repo.path(), knowledge).expect("read").expect("kept");
  assert!(stopped.runs().is_empty());
}
