//! A local MCP server is a connection with its own id, not a versioned resource: what uses it is the profiles whose
//! latest version chooses it and the runs whose pinned profile versions choose it, which are the runs its archive
//! stops.

use orchester_api::SharedResourceKind;
use orchester_core::{
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  id::{ConnectionId, RunId},
  resource::CommandProgram,
  store::McpServerStore,
};
use orchester_workspace::{
  sharing::sync::pinning_mcp_server,
  used_by::{UsedByError, used_by_mcp_server},
};

use super::{
  shown,
  support::{Fixture, at},
};

fn server(fixture: &mut Fixture, label: &str) -> ConnectionId {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(label).expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("docs-mcp").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    at(1),
  )
  .expect("server");
  fixture.db.create_mcp_server(&server).expect("stored");
  server.id()
}

#[test]
fn used_by_local_mcp_lists_the_profile_that_chooses_it_and_the_run_that_uses_it() {
  let mut fixture = Fixture::new();
  let (chosen, other) = (server(&mut fixture, "figma"), server(&mut fixture, "docs"));
  fixture.choose_local_server(chosen);
  let task = fixture.task("Design", at(10));
  let run = fixture.started_run(task, at(11));
  let runtime = tokio::runtime::Runtime::new().expect("runtime");

  let users = runtime
    .block_on(used_by_mcp_server(fixture.repo.path(), chosen))
    .expect("what uses the server");
  assert_eq!(
    shown(&users),
    (vec![(SharedResourceKind::AgentProfile, "Builder", 2)], vec!["ORC-1"]),
    "the profile's latest version chooses it, and the run pinned that version"
  );
  let unused = runtime
    .block_on(used_by_mcp_server(fixture.repo.path(), other))
    .expect("what uses the server");
  assert_eq!(shown(&unused), (Vec::new(), Vec::new()));
  let stopped = pinning_mcp_server(fixture.repo.path(), chosen)
    .expect("read")
    .expect("kept");
  assert_eq!(stopped.runs(), [(run.id(), task)], "the list is what the archive stops");
  assert_eq!(stopped.resource().as_str(), "figma");
}

/// Rows in a run's inputs tables that cannot be read, as a database damaged outside the app leaves them.
fn damage_inputs(fixture: &Fixture, run: RunId) {
  let database = rusqlite::Connection::open(fixture.repo.path().join(".orchester/orchester.db")).expect("open");
  let run = run.to_string();
  database
    .execute(
      "INSERT INTO run_bundles (run_id, origin, sha256, body) VALUES (?1, 'local', ?2, 'not json')",
      [run.as_str(), &"0".repeat(64)],
    )
    .expect("a bundle");
  for table in ["run_local_bindings", "run_local_consent"] {
    database
      .execute(
        &format!("INSERT INTO {table} (run_id, body) VALUES (?1, 'not json')"),
        [run.as_str()],
      )
      .expect("a row");
  }
}

#[test]
fn used_by_local_mcp_a_run_with_unreadable_inputs_does_not_fail_the_list_or_the_archive() {
  let mut fixture = Fixture::new();
  let chosen = server(&mut fixture, "figma");
  fixture.choose_local_server(chosen);
  let task = fixture.task("Design", at(10));
  let damaged = fixture.started_run(task, at(11));
  damage_inputs(&fixture, damaged.id());
  let runtime = tokio::runtime::Runtime::new().expect("runtime");

  let users = runtime
    .block_on(used_by_mcp_server(fixture.repo.path(), chosen))
    .expect("what uses the server");
  assert_eq!(users.runs.len(), 1, "the run's pinned profile still chooses the server");
  let stopped = pinning_mcp_server(fixture.repo.path(), chosen)
    .expect("read")
    .expect("kept");
  assert_eq!(stopped.runs(), [(damaged.id(), task)]);
}

#[test]
fn used_by_local_mcp_of_a_server_the_project_does_not_have_is_not_found() {
  let fixture = Fixture::new();
  let runtime = tokio::runtime::Runtime::new().expect("runtime");
  let unknown = ConnectionId::generate();
  assert!(matches!(
    runtime.block_on(used_by_mcp_server(fixture.repo.path(), unknown)),
    Err(UsedByError::NotFound)
  ));
  assert!(
    pinning_mcp_server(fixture.repo.path(), unknown)
      .expect("read")
      .is_none()
  );
}
