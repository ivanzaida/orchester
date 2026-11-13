//! The tools on a team project and on a local one.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, IdempotencyKey, TaskId},
  integration::base::RunBaseSource,
  name::Name,
  resource::{AgentProfile, Pipeline, Slug},
  revision::Revision,
  run::{BranchName, InterruptCause, NewRun, PublishedVersions, Run, RunEvent, RunSnapshot},
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}
use orchester_store::TeamStartStage;
use serde_json::json;

use crate::{
  Checkout,
  fixture::Config,
  reads,
  team_server::{MEMBER, TeamServer},
};

/// The fake team server with the tools' routes, and what they saw.
pub(crate) fn server() -> (TeamServer, std::sync::Arc<parking_lot::Mutex<reads::Seen>>) {
  let config = Config::new("Say hello.");
  let (extra, seen) = reads::routes(&config);
  (TeamServer::start_with(config, Some(extra)), seen)
}

#[test]
fn team_mcp_reads_come_from_the_team_server() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  let task = checkout
    .call("get_task", &json!({"project": "Kontur", "key": "kon-7"}))
    .expect("read");
  assert_eq!(task["title"], "Say hello");
  assert_eq!(task["id"], server.task.to_string());
  assert_eq!(task["module"], server.module.to_string());
  assert_eq!(task["revision"], 1);
  assert_eq!(task["team"], true);
  // The server sends no record of who closed it or set a reason (an open task has none): both read as null.
  assert!(task["closed"].is_null() && task["blocked"].is_null(), "{task}");
  assert!(
    task
      .as_object()
      .is_some_and(|task| task.contains_key("closed") && task.contains_key("blocked")),
    "the fields are there, null, not left out: {task}"
  );
  let by_id = checkout
    .call(
      "get_task",
      &json!({"project": "Kontur", "key": server.task.to_string()}),
    )
    .expect("read by id");
  assert_eq!(by_id["key"], "KON-7");

  let listed = checkout
    .call("list_tasks", &json!({"project": "Kontur"}))
    .expect("listed");
  let keys: Vec<&str> = listed["tasks"]
    .as_array()
    .expect("tasks")
    .iter()
    .filter_map(|task| task["key"].as_str())
    .collect();
  assert_eq!(keys, ["KON-7"]);
  let module = checkout
    .call("get_module", &json!({"project": "Kontur", "module": "webhooks"}))
    .expect("the module");
  assert_eq!(module["branch"], crate::team_server::MODULE_BRANCH);
  assert_eq!(module["module"]["id"], server.module.to_string());

  // Across projects, a team project's tasks are not read from this computer's copy: it is listed apart.
  let across = checkout.call("list_tasks", &json!({})).expect("listed");
  assert_eq!(across["tasks"], json!([]));
  assert_eq!(
    across["unreadable_projects"][0]["error"]["code"],
    "team_project_unsupported"
  );
  assert!(
    checkout.db().tasks().expect("tasks").is_empty(),
    "no copy of the team's tasks"
  );
}

#[test]
fn team_mcp_reads_say_who_closed_and_who_blocked_a_team_task() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let person = json!({"id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Dana"});
  let read = || {
    checkout
      .call("get_task", &json!({"project": "Kontur", "key": "kon-7"}))
      .expect("read")
  };

  seen.lock().task_fields = json!({
    "workflow": "done",
    "closed": {"how": {"kind": "person", "by": person}, "at": 1_700_000_000_000_i64}
  });
  let closed = read();
  assert_eq!(
    closed["closed"],
    json!({"how": {"kind": "person", "by": person}, "at": 1_700_000_000_000_i64}),
    "{closed}"
  );
  assert!(closed["blocked"].is_null(), "{closed}");

  seen.lock().task_fields = json!({
    "blocked_reason": "Waiting for the staging key",
    "blocked": {"by": person, "at": 1_700_000_060_000_i64}
  });
  let blocked = read();
  assert_eq!(
    blocked["blocked"],
    json!({"by": person, "at": 1_700_000_060_000_i64}),
    "{blocked}"
  );
  assert!(blocked["closed"].is_null(), "{blocked}");
}

#[test]
fn team_mcp_member_edit_is_refused_as_not_admin_and_changes_nothing() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  let edit = json!({"project": "Kontur", "key": "KON-7", "title": "Say goodbye"});
  assert_eq!(checkout.refused("update_task", &edit), "not_admin");
  assert_eq!(
    seen.lock().task_edits,
    1,
    "the edit went to the task's definition route"
  );
  let create = json!({"project": "Kontur", "title": "Say goodbye"});
  assert_eq!(checkout.refused("create_task", &create), "not_admin");
  assert_eq!(seen.lock().task_creates, 1, "the new task went to task.create");
  let task = checkout
    .call("get_task", &json!({"project": "Kontur", "key": "KON-7"}))
    .expect("read");
  assert_eq!(task["title"], "Say hello");
  assert!(checkout.db().tasks().expect("tasks").is_empty(), "nothing saved here");

  // A member is no holder here: the server refuses the close (the stand-in's rules are in `team_lifecycle`), and what
  // was sent is the lifecycle change alone. The limits wait for D6 and are not sent.
  let done = json!({"project": "Kontur", "key": "KON-7", "workflow_status": "done"});
  assert_eq!(checkout.refused("update_task", &done), "not_admin");
  let sent = seen.lock().edited_changes.clone();
  assert_eq!(sent["workflow"], "done");
  assert!(
    sent["title"].is_null() && sent["priority"].is_null(),
    "the lifecycle change alone: {sent}"
  );
  let budget = json!({"project": "Kontur", "key": "KON-7", "budget_tokens": 1000});
  assert_eq!(checkout.refused("update_task", &budget), "decision_required");
  let points = json!({"project": "Kontur", "key": "KON-7", "story_points": 3});
  assert_eq!(checkout.refused("update_task", &points), "team_field_unsupported");
  assert_eq!(seen.lock().task_edits, 2, "only the Done reached the server");
  let draft = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("return_to_draft", &draft), "team_no_queue");
}

#[test]
fn team_mcp_an_update_with_a_refused_change_sends_nothing() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  // The title alone would go to the server; In progress is no lifecycle change of a team task, so neither is sent.
  let edit = json!({"project": "Kontur", "key": "KON-7", "title": "Say goodbye", "workflow_status": "in_progress"});
  assert_eq!(checkout.refused("update_task", &edit), "team_status_not_offered");
  let create = json!({"project": "Kontur", "title": "Say goodbye", "workflow_status": "done"});
  assert_eq!(checkout.refused("create_task", &create), "team_status_on_create");
  let seen = seen.lock();
  assert_eq!(seen.task_edits, 0, "no edit reached the server");
  assert_eq!(seen.task_creates, 0, "no task was created");
}

#[test]
fn team_mcp_a_message_does_not_run_a_team_task_again() {
  let team = crate::team_runs::started();
  team.settle(|run, now| run.cancel(now).expect("canceled"));

  // A canceled run of a team task: a message would run it again, which only the team server's work start may do.
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Try again."});
  let error = team.checkout.call("send_message", &message).expect_err("refused");
  assert_eq!(error["error"]["code"], "team_run_again_unsupported", "{error}");
  assert_eq!(
    team.server.state().works_made,
    1,
    "nothing started beyond the task's first work"
  );
  assert_eq!(team.messages(), 0, "the message was not kept");
}

/// A run of the project's first task with a one-step pipeline published here, started and canceled.
pub(crate) fn canceled_run(db: &mut orchester_store::ProjectDb) {
  run_from(db, (None, RunBaseSource::CheckedOut));
}

/// [`canceled_run`], started from `base_branch` as `base_source` says it was chosen.
pub(crate) fn run_from(db: &mut orchester_store::ProjectDb, base: (Option<&str>, RunBaseSource)) {
  ended_run(db, base, |run, now| run.cancel(now).expect("canceled"));
}

/// [`canceled_run`], with the run interrupted instead.
pub(crate) fn interrupted_run(db: &mut orchester_store::ProjectDb) {
  ended_run(db, (None, RunBaseSource::CheckedOut), |run, now| {
    run.interrupt(InterruptCause::RunnerLost, now).expect("interrupted")
  });
}

fn ended_run(
  db: &mut orchester_store::ProjectDb,
  base: (Option<&str>, RunBaseSource),
  end: impl FnOnce(&mut Run, Timestamp) -> Vec<RunEvent>,
) {
  let task = db.tasks().expect("tasks")[0].id();
  let mut run = started_run(db, task, base);
  let events = end(&mut run, Timestamp::now());
  db.append_events(&run, &events).expect("ended run");
}

/// A run of `task` of the old sharing stream: started with a one-step pipeline published here, and with no inputs kept.
pub(crate) fn started_run(
  db: &mut orchester_store::ProjectDb,
  task: TaskId,
  (base_branch, base_source): (Option<&str>, RunBaseSource),
) -> Run {
  let now = Timestamp::now();
  let builder = crate::fixture::builder();
  let profile = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      name("Builder"),
      builder.body(),
      now,
    )
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    profile,
    db.publish_draft::<AgentProfile>(profile, now).expect("publish profile"),
  )]);
  let pipeline = crate::fixture::one_step(profile, "Say hello.");
  let id = db
    .create_published(
      Slug::new("delivery").expect("slug"),
      name("Delivery"),
      pipeline.body(),
      now,
    )
    .expect("pipeline")
    .0
    .id();
  let version = db
    .latest_version::<Pipeline>(id)
    .expect("publish pipeline")
    .expect("published");
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let new = NewRun {
    task,
    snapshot: RunSnapshot::build(id, &version, published).expect("snapshot"),
    branch: BranchName::new("orchester/kon-1-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: base_branch.map(|branch| BranchName::new(branch).expect("branch")),
    base_source,
    budget: None,
    max_duration: None,
  };
  let mut run = db.create_run(IdempotencyKey::generate(), new, now).expect("run");
  let events = run.start(now).expect("started");
  db.append_events(&run, &events).expect("started run");
  run
}

#[test]
fn team_mcp_run_task_on_a_team_task_goes_through_work_start() {
  let (server, _seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });

  // The session has no runner here: the start is accepted and kept, and waits for the runner.
  let code = checkout.refused("run_task", &json!({"project": "Kontur", "key": "KON-7"}));
  assert_eq!(code, "runner_unavailable");
  {
    let state = server.state();
    assert_eq!(state.works_made, 1, "work.start made the task's work");
    assert!(
      state.claim.as_ref().is_some_and(|claim| claim.token == MEMBER),
      "claimed for this computer's session"
    );
  }
  let starts = checkout.db().open_team_starts().expect("starts");
  assert_eq!(starts.len(), 1);
  assert_eq!(
    starts[0].stage,
    TeamStartStage::InputsStored,
    "the accepted inputs are kept"
  );
  assert!(
    checkout.db().tasks().expect("tasks").is_empty(),
    "no task definition kept"
  );

  // A second Codex connection and no pick for the profile: the pipeline is not ready here, and nothing more starts.
  let second = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex work").expect("name"),
    AgentProvider::CodexCli,
    checkout.root().join("codex-work.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  checkout.db().create_connection(&second).expect("a connection");
  let code = checkout.refused("run_task", &json!({"project": "Kontur", "key": "KON-7"}));
  assert_eq!(code, "not_ready_here");
  assert_eq!(server.state().works_made, 1, "nothing more was started");
}

#[test]
fn team_mcp_no_sql_fallback_while_the_server_is_unavailable() {
  let (server, _seen) = server();
  // A draft saved before the project was linked: a task this computer's database still holds.
  let checkout = Checkout::new(Some(&server), &["Say hello"]);
  assert_eq!(checkout.db().tasks().expect("tasks").len(), 1);

  let local = json!({"project": "Kontur", "key": "KON-1"});
  assert_eq!(
    checkout.refused("get_task", &local),
    "task_not_found",
    "only the server's tasks are read"
  );

  server.set_down(true);
  let task = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("get_task", &task), "team_unavailable");
  assert_eq!(checkout.refused("get_task", &local), "team_unavailable");
  assert_eq!(
    checkout.refused("list_tasks", &json!({"project": "Kontur"})),
    "team_unavailable"
  );
  assert_eq!(
    checkout.refused("list_modules", &json!({"project": "Kontur"})),
    "team_unavailable"
  );
  assert_eq!(checkout.refused("run_task", &task), "team_unavailable");
  assert_eq!(server.state().works_made, 0, "nothing started");
  let edit = json!({"project": "Kontur", "key": "KON-7", "title": "Say goodbye"});
  assert_eq!(checkout.refused("update_task", &edit), "team_unavailable");
  assert_eq!(
    checkout.db().tasks().expect("tasks")[0].details().title.as_str(),
    "Say hello",
    "nothing changed here"
  );
}

#[test]
fn team_mcp_local_projects_are_unchanged() {
  let checkout = Checkout::new(None, &["Sign in"]);

  let task = checkout
    .call("get_task", &json!({"project": "Kontur", "key": "KON-1"}))
    .expect("read");
  assert_eq!(task["title"], "Sign in");
  assert!(task.get("team").is_none());
  let created = checkout
    .call("create_task", &json!({"project": "Kontur", "title": "Sign out"}))
    .expect("created");
  assert_eq!(created["task"]["key"], "KON-2");
  let updated = checkout
    .call(
      "update_task",
      &json!({"project": "Kontur", "key": "KON-2", "title": "Sign out everywhere", "workflow_status": "todo"}),
    )
    .expect("updated");
  assert_eq!(updated["task"]["workflow_status"], "todo");
  let listed = checkout
    .call("list_tasks", &json!({"project": "Kontur"}))
    .expect("listed");
  assert_eq!(listed["tasks"].as_array().expect("tasks").len(), 2);
  let titles: Vec<String> = checkout
    .db()
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.details().title.as_str().to_owned())
    .collect();
  assert_eq!(titles, ["Sign in", "Sign out everywhere"]);
  let bindings = json!({"project": "Kontur", "key": "KON-1", "bindings": [{"profile": "x", "connection": "Codex"}]});
  assert_eq!(checkout.refused("run_task", &bindings), "invalid_arguments");
}

#[test]
fn team_mcp_a_member_cancelling_is_refused_in_the_lifecycles_words() {
  let (server, _) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let reasons = |args: serde_json::Value| {
    let error = checkout.call("update_task", &args).expect_err("refused");
    assert_eq!(error["error"]["code"], "not_admin", "{error}");
    error["error"]["reasons"].clone()
  };

  let cancel = json!({"project": "Kontur", "key": "KON-7", "workflow_status": "canceled"});
  assert_eq!(reasons(cancel), json!(["appMcp.errors.teamLifecycleNotAdmin"]));
  let reopen = json!({"project": "Kontur", "key": "KON-7", "workflow_status": "backlog"});
  assert_eq!(reasons(reopen), json!(["appMcp.errors.teamLifecycleNotAdmin"]));
  // A definition edit is still the team configuration's refusal.
  let edit = json!({"project": "Kontur", "key": "KON-7", "title": "Say goodbye"});
  assert_eq!(reasons(edit), json!(["sharing.errors.notAdmin"]));
}

#[test]
fn team_mcp_a_member_cancelling_with_other_changes_is_refused_by_the_change_the_server_met() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let reasons = |args: serde_json::Value| {
    let error = checkout.call("update_task", &args).expect_err("refused");
    assert_eq!(error["error"]["code"], "not_admin", "{error}");
    error["error"]["reasons"].clone()
  };

  // The details go on their own patch, before the organization's: the title is what the server refused, and the cancel
  // was not sent.
  let with_title = json!({"project": "Kontur", "key": "KON-7", "workflow_status": "canceled", "title": "Say goodbye"});
  assert_eq!(reasons(with_title), json!(["sharing.errors.notAdmin"]));
  assert_eq!(seen.lock().task_edits, 1, "only the title reached the server");
  assert!(seen.lock().edited_changes["workflow"].is_null());

  // The organization's fields share one patch, so the refusal does not say which of the two it met.
  let with_priority = json!({"project": "Kontur", "key": "KON-7", "workflow_status": "canceled", "priority": "high"});
  assert_eq!(
    reasons(with_priority),
    json!(["appMcp.errors.teamLifecycleNotAdminWithDetails"])
  );
  assert_eq!(seen.lock().edited_changes["workflow"], "canceled");
}
