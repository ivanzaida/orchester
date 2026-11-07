//! What `run_task` chooses for a team start (`team_work/choices.rs`): a profile runs on a connection of the CLI kind
//! its version names, never on one of another CLI; a body that is not a team profile (it names no CLI kind) is refused;
//! and a team MCP server a profile names is refused unless this computer has the stopgap's server for it (ORC-826
//! replaces that rule).

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  name::Name,
  store::{ConnectionStore, McpServerStore},
  time::Timestamp,
};
use serde_json::json;

use crate::{Checkout, pipelines::team_pipeline, reads, team_server::TeamServer, tools::local_mcp_server};

/// The fake team server whose Builder profile version names `cli`, and what the routes saw.
fn server_running(cli: Option<AgentProvider>) -> (TeamServer, std::sync::Arc<parking_lot::Mutex<reads::Seen>>) {
  let config = crate::fixture::Config::new("Say hello.").running(cli.unwrap_or(AgentProvider::CodexCli));
  let (extra, seen) = reads::routes_running(&config, cli);
  (TeamServer::start_with(config, Some(extra)), seen)
}

fn connection(checkout: &Checkout, provider: AgentProvider, label: &str) {
  let made = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(label).expect("name"),
    provider,
    checkout.root().join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  checkout.db().create_connection(&made).expect("a connection");
}

#[test]
fn team_mcp_choices_a_profile_runs_only_on_a_connection_of_its_cli_kind() {
  let (server, seen) = server_running(Some(AgentProvider::ClaudeCodeCli));
  let pipeline = server.state().config.pipeline;
  let mcp_server = seen.lock().publisher_mcp_server.expect("the Builder's MCP server");
  // This computer has a Codex connection (the checkout's own) and the MCP server, but no Claude Code connection.
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, pipeline, "delivery");
    db.create_mcp_server(&local_mcp_server(mcp_server))
      .expect("a local MCP server");
  });

  let run = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("run_task", &run), "bindings_required");
  assert_eq!(
    server.state().works_made,
    0,
    "nothing starts on a connection of another CLI"
  );

  // The only connection of the profile's CLI is used without asking: the body names none.
  connection(&checkout, AgentProvider::ClaudeCodeCli, "claude");
  assert_eq!(checkout.refused("run_task", &run), "runner_unavailable");
  assert_eq!(
    server.state().works_made,
    1,
    "bound to the Claude Code connection and started"
  );
}

#[test]
fn team_mcp_choices_a_profile_body_that_names_no_cli_kind_is_refused() {
  let (server, seen) = server_running(None);
  let pipeline = server.state().config.pipeline;
  let mcp_server = seen.lock().publisher_mcp_server.expect("the Builder's MCP server");
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, pipeline, "delivery");
    db.create_mcp_server(&local_mcp_server(mcp_server))
      .expect("a local MCP server");
  });

  let run = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("run_task", &run), "team_read_failed");
  assert_eq!(
    server.state().works_made,
    0,
    "no CLI is guessed from a body's connection"
  );
}

#[test]
fn team_mcp_choices_a_team_mcp_server_is_not_matched_by_name_to_a_local_one() {
  let (server, seen) = server_running(Some(AgentProvider::CodexCli));
  let pipeline = server.state().config.pipeline;
  assert!(seen.lock().publisher_mcp_server.is_some());
  // A local server with the team server's name but another id does not stand in for it.
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    team_pipeline(db, pipeline, "delivery");
    db.create_mcp_server(&local_mcp_server(ConnectionId::generate()))
      .expect("another local MCP server");
  });

  let run = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("run_task", &run), "mcp_servers_required");
  assert_eq!(
    server.state().works_made,
    0,
    "nothing starts without the server a profile names"
  );
}
