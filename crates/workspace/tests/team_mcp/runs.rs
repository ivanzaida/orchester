//! A team run is read without its task's definition (team authority P7): the tools that take a run by its id or its
//! number answer from the run's own records, and the task's key comes from the inputs the run started with.

use orchester_core::store::{ConversationStore, RunStore};
use serde_json::json;

use crate::team_runs::started;

#[test]
fn team_mcp_get_run_reads_a_run_without_the_task_row() {
  let team = started();
  let id = team.run.to_string();

  let by_id = team
    .checkout
    .call("get_run", &json!({"project": "Kontur", "run": id}))
    .expect("the run by its id");
  assert_eq!(by_id["task"], "KON-7");
  assert_eq!(by_id["id"], id.as_str());
  assert_eq!(by_id["status"]["state"], "running");
  let number = by_id["number"].as_u64().expect("number");

  // The number names a run of a task this computer keeps no row of.
  for named in [number.to_string(), format!("#{number}")] {
    let by_number = team
      .checkout
      .call("get_run", &json!({"project": "Kontur", "run": named}))
      .expect("the run by its number");
    assert_eq!(by_number["id"], id.as_str());
  }
  let missing = team.checkout.refused("get_run", &json!({"project": "Kontur", "run": "999"}));
  assert_eq!(missing, "run_not_found");
}

#[test]
fn team_mcp_send_message_reaches_a_team_run_without_the_task_row() {
  let team = started();
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Use the webhooks module."});

  // The message is stored in the task's conversation; the session has no runner, so it stays queued with why.
  let sent = team.checkout.call("send_message", &message).expect("sent");
  assert_eq!(sent["run"]["id"], team.run.to_string().as_str(), "{sent}");
  assert!(sent["message"]["id"].is_string(), "{sent}");
  let task = team.checkout.db().run(team.run).expect("read").expect("run").task();
  let kept = team.checkout.db().messages(task).expect("messages");
  assert_eq!(kept.len(), 1, "the message was kept in the team task's conversation");
  assert_eq!(kept[0].body().as_str(), "Use the webhooks module.");
}
