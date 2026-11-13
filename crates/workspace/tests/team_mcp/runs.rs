//! A team run is read without its task's definition (team authority P7): the tools that take a run by its id or its
//! number answer from the run's own records, and the task's key comes from the inputs the run started with.

use orchester_core::store::{ConversationStore, RunStore, TaskStore};
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
  let missing = team
    .checkout
    .refused("get_run", &json!({"project": "Kontur", "run": "999"}));
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

#[test]
fn team_mcp_conversation_run_lists_and_approvals_name_a_team_task_without_its_row() {
  let team = started();
  let project = json!({"project": "Kontur"});

  let conversation = team
    .checkout
    .call("get_conversation", &json!({"project": "Kontur", "task": "KON-7"}))
    .expect("the conversation");
  assert_eq!(conversation["task"]["key"], "KON-7");
  assert_eq!(conversation["task"]["title"], "Say hello");
  assert_eq!(conversation["run"]["id"], team.run.to_string().as_str());

  // With no task given the team run is listed, as it is with the task named by its key or its server id.
  for args in [
    project.clone(),
    json!({"project": "Kontur", "task": "kon-7"}),
    json!({"project": "Kontur", "task": team.server.task.to_string()}),
  ] {
    let listed = team.checkout.call("list_runs", &args).expect("listed");
    let runs = listed["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 1, "{args}: {listed}");
    assert_eq!(runs[0]["id"], team.run.to_string().as_str());
    assert_eq!(runs[0]["task"]["key"], "KON-7");
    assert_eq!(runs[0]["task"]["title"], "Say hello");
  }
  assert_eq!(
    team
      .checkout
      .refused("list_runs", &json!({"project": "Kontur", "task": "KON-99"})),
    "task_not_found"
  );

  let approvals = team.checkout.call("get_approvals", &project).expect("approvals");
  assert_eq!(approvals["approvals"]["runs"][0]["task"], "KON-7", "{approvals}");
}

#[test]
fn team_mcp_a_team_task_never_run_here_has_an_empty_conversation_and_no_group() {
  let (server, _seen) = crate::tools::server();
  let checkout = crate::Checkout::new(Some(&server), &[]);

  let conversation = checkout
    .call("get_conversation", &json!({"project": "Kontur", "task": "KON-7"}))
    .expect("the conversation");
  assert_eq!(conversation["task"]["key"], "KON-7");
  assert_eq!(conversation["run"], serde_json::Value::Null);
  assert_eq!(conversation["entries"], json!([]));
  let runs = checkout
    .call("list_runs", &json!({"project": "Kontur", "task": "KON-7"}))
    .expect("listed");
  assert_eq!(runs["runs"], json!([]));

  let hold = json!({"project": "Kontur", "task": "KON-7"});
  assert_eq!(checkout.refused("hold_queue", &hold), "not_in_group");
  assert_eq!(checkout.refused("release_queue", &hold), "not_in_group");
  let unknown = json!({"project": "Kontur", "task": "KON-99"});
  assert_eq!(checkout.refused("hold_queue", &unknown), "task_not_found");
  assert_eq!(
    checkout.refused("get_conversation", &json!({"project": "Kontur", "task": "KON-99"})),
    "task_not_found"
  );
}

#[test]
fn team_mcp_delivery_of_a_team_module_is_refused_not_missing() {
  let (server, _seen) = crate::tools::server();
  let checkout = crate::Checkout::new(Some(&server), &[]);

  // Without a module nothing is computed from the rows this computer keeps of a team's tasks.
  assert_eq!(
    checkout.refused("get_delivery", &json!({"project": "Kontur"})),
    "team_project_unsupported"
  );
  let webhooks = json!({"project": "Kontur", "module": "webhooks"});
  assert_eq!(checkout.refused("get_delivery", &webhooks), "team_project_unsupported");
  let unknown = json!({"project": "Kontur", "module": "nothing"});
  assert_eq!(checkout.refused("get_delivery", &unknown), "module_not_found");
}

#[test]
fn team_mcp_resume_run_takes_no_old_claim_and_continues_nothing_locally() {
  let team = started();
  team.settle(|run, now| {
    run
      .interrupt(orchester_core::run::InterruptCause::RunnerLost, now)
      .expect("interrupted")
  });

  let resume = json!({"project": "Kontur", "run": team.run.to_string()});
  assert_eq!(team.checkout.refused("resume_run", &resume), "team_project_unsupported");
  assert!(
    team.checkout.db().sharing().expect("link").is_some(),
    "still a team project"
  );
  let run = team.checkout.call("get_run", &resume).expect("the run");
  assert_eq!(run["status"]["state"], "interrupted", "the run was not continued");
  assert!(
    team.checkout.db().tasks().expect("tasks").is_empty(),
    "no claim of the old sharing stream cached the task here"
  );
}

/// A team's profile has no row in a shared project's database (`docs/design/team-configuration-live.md`): the step's
/// agent is the profile the run's own inputs pinned.
#[test]
fn team_mcp_get_run_names_the_agent_of_a_profile_the_database_does_not_hold() {
  let team = started();
  let read = |team: &crate::team_runs::Started| {
    let run = team
      .checkout
      .call("get_run", &json!({"project": "Kontur", "run": team.run.to_string()}))
      .expect("the run");
    run["steps"][0]["agent"].as_str().map(str::to_owned)
  };
  let cached = read(&team).expect("the agent of the cached profile");

  let profile = team.server.state().config.profile.to_string();
  let database = rusqlite::Connection::open(team.checkout.root().join(".orchester/orchester.db")).expect("open");
  database
    .execute_batch(&format!(
      "DELETE FROM shared_version_references WHERE version_id IN
         (SELECT id FROM resource_versions WHERE resource_id = '{profile}');
       DELETE FROM shared_version_bodies WHERE version_id IN
         (SELECT id FROM resource_versions WHERE resource_id = '{profile}');
       DROP TRIGGER resource_versions_no_delete;
       DELETE FROM resource_versions WHERE resource_id = '{profile}';
       DELETE FROM resources WHERE id = '{profile}';"
    ))
    .expect("the profile leaves the database");
  drop(database);

  assert_eq!(read(&team), Some(cached));
}
