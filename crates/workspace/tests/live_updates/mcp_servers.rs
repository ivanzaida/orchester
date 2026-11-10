//! The team's MCP servers through the stream (`docs/design/team-configuration.md`, "Who owns what"): a team MCP server
//! is a resource of its own kind, cached with its versions like the team's other resources and never written into this
//! computer's `mcp_servers` table, where the person's own servers live. It follows `resource.published`, whose latest
//! version is the one used here; archived on the server it leaves the list and its runs stop, restored it is back.

use std::sync::Arc;

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, TeamMcpLaunch, TeamMcpServer, VersionNumber},
  store::{McpServerStore, ResourceStore},
};
use orchester_workspace::sharing::sync::{SystemClock, follow};
use serde_json::{Value, json};

use super::{ALICE, Member, SETTLED, TeamServer, Workspace, seen_within};

fn program(command: &str, args: &[&str]) -> Value {
  json!({
    "name": "knowledge",
    "launch": {"kind": "program", "program": command, "args": args, "variables": ["TOKEN"]},
    "exempt_tools": ["search"]
  })
}

fn listed(workspace: &Workspace, server: ResourceId) -> bool {
  let servers = workspace.db().resources(ResourceKind::McpServer).expect("listed");
  servers.iter().any(|kept| kept.id() == server)
}

fn latest(workspace: &Workspace, server: ResourceId) -> Option<VersionNumber> {
  let version = workspace.db().latest_version::<TeamMcpServer>(server).ok().flatten();
  version.map(|version| version.number())
}

fn version_two() -> VersionNumber {
  VersionNumber::FIRST.next().expect("a version after the first")
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_team_mcp_server_is_cached_with_its_versions_and_not_written_to_the_local_servers() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let knowledge = ResourceId::generate();
  server.publish(
    (knowledge, SharedResourceKind::McpServer, "Knowledge"),
    program("npx", &["-y"]),
    Vec::new(),
  );
  let workspace = Workspace::linked(&server);
  let following = workspace.follow(Member::of(&server, ALICE));
  seen_within(SETTLED, "the snapshot's MCP server", || {
    latest(&workspace, knowledge).is_some()
  })
  .await;

  let db = workspace.db();
  let kept = db.resource(knowledge).expect("read").expect("the resource is kept");
  assert_eq!(kept.kind(), ResourceKind::McpServer);
  assert!(listed(&workspace, knowledge));
  let cached = db
    .cached_version::<TeamMcpServer>(knowledge, VersionNumber::FIRST)
    .expect("read")
    .expect("cached");
  assert_eq!(
    cached.server_body,
    program("npx", &["-y"]),
    "the server's body as it is"
  );
  let TeamMcpLaunch::Program(launch) = cached.local_body.launch() else {
    panic!("a program");
  };
  assert_eq!(launch.program().as_str(), "npx");

  // The next version reaches the cache through `resource.published`, and is the one this computer uses at once: no
  // version waits for a person here any more.
  server.publish(
    (knowledge, SharedResourceKind::McpServer, "Knowledge"),
    program("uvx", &[]),
    Vec::new(),
  );
  seen_within(SETTLED, "v2 cached", || {
    workspace
      .db()
      .cached_version::<TeamMcpServer>(knowledge, version_two())
      .ok()
      .flatten()
      .is_some()
  })
  .await;
  assert_eq!(latest(&workspace, knowledge), Some(version_two()));

  assert!(
    workspace.db().mcp_servers().expect("read").is_empty(),
    "a team MCP server is never a local one"
  );
  following.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_an_archived_team_mcp_server_leaves_the_list_and_stops_its_runs_until_restored() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let knowledge = ResourceId::generate();
  server.publish(
    (knowledge, SharedResourceKind::McpServer, "Knowledge"),
    program("npx", &[]),
    Vec::new(),
  );
  let workspace = Workspace::linked(&server);
  let member = Arc::new(Member::of(&server, ALICE));
  let following = {
    let (member, root) = (member.clone(), workspace.root.clone());
    tokio::spawn(async move { follow(&*member, &root, &SystemClock).await })
  };
  seen_within(SETTLED, "the snapshot's MCP server", || listed(&workspace, knowledge)).await;

  server.archive(knowledge, true);
  seen_within(SETTLED, "the MCP server leaves the list", || {
    !listed(&workspace, knowledge)
  })
  .await;
  assert_eq!(
    member.archived.lock().expect("archived").as_slice(),
    [(knowledge, Some("Alice Moreau".to_owned()))],
    "its runs are stopped once"
  );
  assert!(
    latest(&workspace, knowledge).is_some(),
    "its versions stay for the runs that pinned them"
  );

  server.archive(knowledge, false);
  seen_within(SETTLED, "the MCP server is back", || listed(&workspace, knowledge)).await;
  assert_eq!(
    member.archived.lock().expect("archived").as_slice(),
    [(knowledge, Some("Alice Moreau".to_owned()))],
    "a restore stops nothing"
  );
  assert!(workspace.db().mcp_servers().expect("read").is_empty());
  following.abort();
}
