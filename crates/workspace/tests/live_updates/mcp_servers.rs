//! The team's MCP servers through the stream (`docs/design/team-configuration.md`, "Who owns what"): a team MCP server
//! is a resource of its own kind, read into memory with the team's other resources and never written into this
//! computer's `mcp_servers` table, where the person's own servers live, nor anywhere else in its database. It follows
//! `resource.published`, whose latest version is the one used here; archived on the server it leaves the list and its
//! runs stop, restored it is back.

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

/// Whether the team's configuration `member`'s stream read has `server`, not archived.
fn listed((member, workspace): (&Member, &Workspace), server: ResourceId) -> bool {
  member
    .configs
    .read(&workspace.root)
    .config()
    .is_some_and(|config| config.live(server).is_some())
}

/// The latest version of `server` the stream read, with its body.
fn latest((member, workspace): (&Member, &Workspace), server: ResourceId) -> Option<(VersionNumber, TeamMcpServer)> {
  let config = member.configs.read(&workspace.root).config().cloned()?;
  let resource = config.resource(server)?;
  Some((resource.number()?, resource.body::<TeamMcpServer>()?.ok()?))
}

/// Nothing of the team's in the project's database, and no local MCP server made of the team's.
fn nothing_kept(workspace: &Workspace) {
  let db = workspace.db();
  assert!(
    db.resources(ResourceKind::McpServer).expect("read").is_empty(),
    "no team resource kept here"
  );
  assert!(
    db.mcp_servers().expect("read").is_empty(),
    "a team MCP server is never a local one"
  );
}

fn version_two() -> VersionNumber {
  VersionNumber::FIRST.next().expect("a version after the first")
}

fn following(
  workspace: &Workspace,
  member: &Arc<Member>,
) -> tokio::task::JoinHandle<orchester_workspace::sharing::sync::Stopped> {
  let (member, root) = (member.clone(), workspace.root.clone());
  tokio::spawn(async move { follow(&*member, &root, &SystemClock).await })
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
  let member = Arc::new(Member::of(&server, ALICE));
  let stream = following(&workspace, &member);
  seen_within(SETTLED, "the snapshot's MCP server", || {
    latest((&member, &workspace), knowledge).is_some()
  })
  .await;
  let (number, read) = latest((&member, &workspace), knowledge).expect("read");
  assert_eq!(number, VersionNumber::FIRST);
  let TeamMcpLaunch::Program(launch) = read.launch() else {
    panic!("a program");
  };
  assert_eq!(launch.program().as_str(), "npx");

  // The next version is read through `resource.published`, and is the one this computer uses at once.
  server.publish(
    (knowledge, SharedResourceKind::McpServer, "Knowledge"),
    program("uvx", &[]),
    Vec::new(),
  );
  seen_within(SETTLED, "v2 read", || {
    latest((&member, &workspace), knowledge).is_some_and(|(number, _)| number == version_two())
  })
  .await;
  nothing_kept(&workspace);
  stream.abort();
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
  let stream = following(&workspace, &member);
  seen_within(SETTLED, "the snapshot's MCP server", || {
    listed((&member, &workspace), knowledge)
  })
  .await;

  server.archive(knowledge, true);
  seen_within(SETTLED, "the MCP server leaves the list", || {
    !listed((&member, &workspace), knowledge)
  })
  .await;
  assert_eq!(
    member.archived.lock().expect("archived").as_slice(),
    [(knowledge, Some("Alice Moreau".to_owned()))],
    "its runs are stopped once"
  );

  server.archive(knowledge, false);
  seen_within(SETTLED, "the MCP server is back", || {
    listed((&member, &workspace), knowledge)
  })
  .await;
  assert_eq!(
    member.archived.lock().expect("archived").as_slice(),
    [(knowledge, Some("Alice Moreau".to_owned()))],
    "a restore stops nothing"
  );
  nothing_kept(&workspace);
  stream.abort();
}
