//! A local task of a team-linked project (a draft made before the project was linked) is read from this computer's
//! records by the tools that read a task by name, signed in or not; a name the project has no local task for goes to
//! the team server.

use orchester_core::{
  secret::{SecretVault, VaultKey},
  store::{RunStore, TaskStore},
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

/// A local task's run in a team-linked project: made before the project was linked, with no team record.
fn local_run(checkout: &Checkout) -> String {
  let db = checkout.db();
  let task = db.tasks().expect("tasks")[0].id();
  let runs = db.runs_of_task(task).expect("runs");
  runs[0].as_ref().expect("readable").id().to_string()
}

#[test]
fn team_mcp_a_message_to_a_local_run_in_a_team_project_runs_the_task_again_here() {
  for (ended, text) in [
    (
      crate::tools::canceled_run as fn(&mut orchester_store::ProjectDb),
      "Try again.",
    ),
    (crate::tools::interrupted_run, "Go on."),
  ] {
    let (server, _seen) = server();
    let checkout = Checkout::prepared(Some(&server), &["Say hello"], ended);
    let message = json!({"project": "Kontur", "run": local_run(&checkout), "text": text});

    // Not refused as a team task's run: the local path takes the person's claim, which a computer signed out of the
    // server cannot, so the message waits queued with why; the team server was not asked to start any work.
    let sent = checkout.call("send_message", &message).expect("sent");
    assert_eq!(sent["action"], "run_again", "{sent}");
    assert_eq!(sent["outcome"], "queued", "{sent}");
    assert_eq!(sent["reasons"], json!(["servers.errors.signedOut"]), "{sent}");
    assert_eq!(server.state().works_made, 0, "no team work was made");
  }
}

#[test]
fn team_mcp_continue_run_of_a_local_run_in_a_team_project_is_decided_as_a_local_run() {
  let (server, _seen) = server();
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], crate::tools::canceled_run);
  let resume = json!({"project": "Kontur", "run": local_run(&checkout), "choice": {"kind": "resume"}});

  // The local task page's own rule answers (a canceled run was not stopped at a limit), not the team refusal.
  assert_eq!(checkout.refused("continue_run", &resume), "not_stopped_at_limit");
}

#[test]
fn team_mcp_continue_run_of_a_team_run_is_refused_by_its_team_record() {
  let team = crate::team_runs::started();
  let run = team.run.to_string();

  let resume = json!({"project": "Kontur", "run": run, "choice": {"kind": "resume"}});
  assert_eq!(
    team.checkout.refused("continue_run", &resume),
    "team_project_unsupported"
  );
  let raise = json!({
    "project": "Kontur",
    "run": run,
    "choice": {"kind": "raise_limits", "budget_tokens": null, "time_limit_minutes": null},
  });
  assert_eq!(team.checkout.refused("continue_run", &raise), "decision_required");
}

#[test]
fn team_mcp_a_local_task_wins_a_key_the_server_task_has_too_and_the_server_task_is_read_by_its_id() {
  let (server, seen) = server();
  // The server's one task is keyed KON-1 as well, as a task of the team made after the local draft was.
  seen.lock().task_number = 1;
  let checkout = Checkout::new(Some(&server), &["Local draft"]);

  // The key names the local draft, whose definition is a row of this computer.
  let by_key = checkout
    .call("get_conversation", &json!({"project": "Kontur", "task": "KON-1"}))
    .expect("the local draft's conversation");
  assert_eq!(by_key["task"]["title"], "Local draft", "{by_key}");

  // The server's task is reachable by its server id, which no local task has.
  let by_id = checkout
    .call(
      "get_conversation",
      &json!({"project": "Kontur", "task": server.task.to_string()}),
    )
    .expect("the team task's conversation");
  assert_eq!(by_id["task"]["key"], "KON-1", "{by_id}");
  assert_eq!(by_id["task"]["title"], "Say hello", "{by_id}");
}
