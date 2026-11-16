//! The runs a team MCP server's archive stops (`docs/architecture/server.md`, row 79): a run pins no MCP server
//! itself; a team run binds the team MCP servers its pinned profiles choose in its inputs, as the team server accepted
//! them, and that is what an archive reads. The resource is named by the team's configuration read this session. A run
//! with no inputs binding the server — a local run whose profile once chose it by a cached reference — is left alone.
//!
//! `cargo test -p orchester-workspace --test archive_pins archive_pins_`

mod support;

use orchester_core::{id::ResourceId, name::Name, resource::ResourceKind, sharing::TaskRevision};
use orchester_workspace::{
  sharing::sync::pinning,
  team_config::{TeamConfig, TeamResource},
};
use support::{Fixture, at};

/// Team MCP server `id` named `name` as the team's configuration holds it.
fn server(id: ResourceId, name: &str) -> TeamResource {
  TeamResource {
    id,
    kind: ResourceKind::McpServer,
    name: Name::new(name).expect("name"),
    slug: name.to_lowercase(),
    archived: true,
    revision: TaskRevision::try_from(1_u64).expect("a revision"),
    updated: orchester_workspace::team_config::Changed {
      by: "Alice Moreau".to_owned(),
      at: orchester_core::time::Timestamp::from_unix_millis(0),
    },
    latest: None,
  }
}

#[test]
fn archive_pins_a_run_that_binds_no_team_mcp_server_in_its_inputs_is_not_stopped() {
  let (knowledge, other) = (ResourceId::generate(), ResourceId::generate());
  let mut fixture = Fixture::choosing(&[knowledge]);
  fixture.keep_team_server(knowledge, "Knowledge");
  let task = fixture.task("Search", at(10));
  fixture.started_run(task, at(11));
  let config = TeamConfig::new([server(knowledge, "Knowledge"), server(other, "Other")]);

  let root = fixture.repo.path();
  let read = pinning(root, knowledge, Some(&config))
    .expect("read")
    .expect("the team's server");
  assert_eq!(
    read.resource().as_str(),
    "Knowledge",
    "named as the team's configuration names it"
  );
  assert!(
    read.runs().is_empty(),
    "the run's profile chose it only by a cached reference; its inputs bind no team MCP server"
  );
  assert!(
    pinning(root, ResourceId::generate(), Some(&config))
      .expect("read")
      .is_none(),
    "a resource the team's configuration does not hold stops nothing"
  );
}

#[test]
fn archive_pins_a_run_whose_profile_chooses_no_team_mcp_server_is_not_stopped() {
  let knowledge = ResourceId::generate();
  let mut fixture = Fixture::choosing(&[]);
  fixture.keep_team_server(knowledge, "Knowledge");
  let task = fixture.task("Search", at(10));
  fixture.started_run(task, at(11));
  let config = TeamConfig::new([server(knowledge, "Knowledge")]);

  let stopped = pinning(fixture.repo.path(), knowledge, Some(&config))
    .expect("read")
    .expect("the team's server");
  assert!(stopped.runs().is_empty());
}
