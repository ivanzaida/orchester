//! A local task of a team-linked project (a draft made before the project was linked) is read from this computer's
//! records by the tools that read a task by name, signed in or not; a name the project has no local task for goes to
//! the team server.

use orchester_core::{
  secret::{SecretVault, VaultKey},
  store::TaskStore,
};
use serde_json::json;

use crate::{Checkout, team_server::TeamServer, tools::server};

/// A checkout linked to the fake team server, with the local draft `KON-1`; signed out of the server when `signed_in`
/// is not set (the server's token is gone from the keychain).
fn linked(signed_in: bool) -> (Checkout, TeamServer) {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &["Say hello"]);
  if !signed_in {
    let link = checkout.db().sharing().expect("link").expect("linked");
    checkout
      .vault
      .delete(&VaultKey::server_token(link.server))
      .expect("the token is gone");
  }
  (checkout, server)
}

fn local_task_is_read_here(signed_in: bool) {
  let (checkout, _server) = linked(signed_in);
  let local = json!({"project": "Kontur", "task": "KON-1"});

  let conversation = checkout.call("get_conversation", &local).expect("the conversation");
  assert_eq!(conversation["task"]["key"], "KON-1", "{conversation}");
  assert_eq!(conversation["task"]["title"], "Say hello");
  assert_eq!(conversation["run"], serde_json::Value::Null);

  let by_id = json!({"project": "Kontur", "task": checkout.db().tasks().expect("tasks")[0].id().to_string()});
  let runs = checkout.call("list_runs", &by_id).expect("listed");
  assert_eq!(runs["runs"], json!([]), "{runs}");
  assert_eq!(
    checkout.refused("list_runs", &json!({"project": "Kontur", "task": "KON-99"})),
    if signed_in { "task_not_found" } else { "team_signed_out" },
    "a name with no local task goes to the server"
  );

  // The draft is in no reviewed group: the local answer, not the server's.
  for tool in ["hold_queue", "release_queue"] {
    assert_eq!(checkout.refused(tool, &local), "not_in_group", "{tool}");
  }
}

#[test]
fn team_mcp_a_local_task_in_a_team_project_is_read_here_when_signed_in() {
  local_task_is_read_here(true);
}

#[test]
fn team_mcp_a_local_task_in_a_team_project_is_read_here_when_signed_out() {
  local_task_is_read_here(false);
}

#[test]
fn team_mcp_a_team_task_is_still_read_from_the_server_beside_a_local_one() {
  let (checkout, server) = linked(true);
  let runs = checkout
    .call(
      "list_runs",
      &json!({"project": "Kontur", "task": server.task.to_string()}),
    )
    .expect("listed");
  assert_eq!(runs["runs"], json!([]), "{runs}");
  let conversation = checkout
    .call("get_conversation", &json!({"project": "Kontur", "task": "KON-7"}))
    .expect("the team task's conversation");
  assert_eq!(conversation["task"]["key"], "KON-7");
}
