//! A team task of the old sharing stream that this computer can tell nothing of: a run that kept no inputs and no
//! cached row of its task is listed without the task rather than dropped, a team task with no run here is no part of
//! the hold queue, and a team task whose runs kept no inputs is still the row the stream cached.

use std::num::NonZeroU32;

use orchester_core::{
  id::{RunId, TaskId},
  integration::base::RunBaseSource,
  name::Name,
  run::WaitReason,
  sharing::{CachedTask, TaskRevision},
  store::{RunStore, TaskStore},
  task::{TaskLimits, TaskStatus, identity::TeamTaskRef, organization::TaskOrganization},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::StoreError;
use orchester_workspace::tasks::{detail::TaskDetail, hold::held_work, recorded};
use serde_json::json;

use crate::{
  Checkout, Standing,
  team_server::TeamServer,
  tools::{server, started_run},
};

/// A checkout linked to the fake team server, with the team task `KON-7` of the old sharing stream: its identity, the
/// row the stream cached and a started run, which has neither inputs nor a team record. The team's identity of its task
/// does not make it a team run: it is a local run, as before team runs were recorded.
pub(crate) fn stream_era() -> (Checkout, TeamServer, TaskId, RunId) {
  let (checkout, server, task) = cached_without_run();
  let mut db = checkout.db();
  let run = started_run(&mut db, task, (None, RunBaseSource::CheckedOut)).id();
  drop(db);
  (checkout, server, task, run)
}

/// [`stream_era`] before its run started: the team task's identity and the row the stream cached, and no run.
pub(crate) fn cached_without_run() -> (Checkout, TeamServer, TaskId) {
  let (server, _seen) = server();
  let admin = Standing {
    role: "owner",
    widens: false,
  };
  let checkout = Checkout::standing(Some(&server), (&[], admin), |_| {});
  let mut db = checkout.db();
  let link = db.sharing().expect("link").expect("linked");
  let task = db
    .record_team_task(
      TeamTaskRef {
        server: link.server,
        project: link.project,
        task: server.task,
      },
      TaskRevision::try_from(1).expect("revision"),
    )
    .expect("identity");
  db.cache_shared_tasks(&[cached(task)], None).expect("cached");
  drop(db);
  (checkout, server, task)
}

/// [`stream_era`] after the row the stream cached is gone (the cache is dropped when the link is) and the run stays: a
/// team task with no record that defines it.
pub(crate) fn legacy() -> (Checkout, TeamServer, TaskId, RunId) {
  let (checkout, server, task, run) = stream_era();
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute("DELETE FROM tasks WHERE id = ?1", [task.to_string()])
    .expect("the cached row dropped");
  (checkout, server, task, run)
}

/// The cached copy the old stream kept of team task `id`, queued as `KON-7`.
fn cached(id: TaskId) -> CachedTask {
  CachedTask {
    id,
    number: NonZeroU32::new(7).expect("number"),
    revision: TaskRevision::try_from(1).expect("revision"),
    title: Name::new("Cached title").expect("name"),
    brief: "Cached brief.".to_owned(),
    criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
    status: TaskStatus::Queued,
    organization: TaskOrganization::default(),
    module: None,
    former_key: None,
    created_by: "Alice Moreau".to_owned(),
    created_at: Timestamp::now(),
    updated_by: "Alice Moreau".to_owned(),
    updated_at: Timestamp::now(),
    claim: None,
    run: None,
  }
}

#[test]
fn team_mcp_a_legacy_team_run_with_no_inputs_and_no_row_is_listed_without_its_task() {
  let (checkout, _server, task, run) = legacy();
  assert!(checkout.db().task(task).expect("read").is_none(), "no cached row");
  let project = json!({"project": "Kontur"});

  let listed = checkout.call("list_runs", &project).expect("listed");
  let runs = listed["runs"].as_array().expect("runs");
  assert_eq!(runs.len(), 1, "{listed}");
  assert_eq!(runs[0]["id"], run.to_string().as_str());
  assert_eq!(runs[0]["task"], json!(null));
  assert_eq!(runs[0]["unrecorded"], true);
  assert_eq!(runs[0]["status"]["state"], "running");

  let approvals = checkout.call("get_approvals", &project).expect("approvals");
  let running = approvals["approvals"]["runs"].as_array().expect("runs");
  assert_eq!(running.len(), 1, "{approvals}");
  assert_eq!(running[0]["id"], run.to_string().as_str());
  assert_eq!(running[0]["task"], json!(null));
  assert_eq!(running[0]["unrecorded"], true);
}

#[test]
fn team_mcp_a_legacy_team_runs_message_wait_is_listed_without_its_task() {
  let (checkout, _server, _task, run) = legacy();
  let mut db = checkout.db();
  let mut stored = db.run(run).expect("read").expect("run");
  let execution = stored.current_attempt().expect("attempt").executions()[0].id();
  let now = Timestamp::now();
  let mut events = stored.start_step(execution, now).expect("step started");
  events.extend(stored.wait_step(execution, WaitReason::Message, now).expect("waits"));
  db.append_events(&stored, &events).expect("recorded");
  drop(db);

  let listed = checkout
    .call("list_questions", &json!({"project": "Kontur"}))
    .expect("listed");
  let waits = listed["waiting_for_message"].as_array().expect("waits");
  assert_eq!(waits.len(), 1, "{listed}");
  assert_eq!(waits[0]["task"], json!(null));
  assert_eq!(waits[0]["unrecorded"], true);
  assert_eq!(waits[0]["run"]["id"], run.to_string().as_str());
}

#[test]
fn team_mcp_a_legacy_team_task_with_unrecorded_runs_is_refused_by_name_not_shown_empty() {
  let (checkout, server, _task, _run) = legacy();

  // The server knows the task; this computer ran it, but kept nothing that says what it is.
  for tool in ["get_conversation", "list_runs"] {
    let args = json!({"project": "Kontur", "task": server.task.to_string()});
    assert_eq!(checkout.refused(tool, &args), "team_task_unrecorded", "{tool}");
  }
}

#[test]
fn team_mcp_a_team_task_with_runs_that_kept_no_inputs_is_the_row_the_stream_cached() {
  let (checkout, _server, task, run) = legacy();
  checkout.db().cache_shared_tasks(&[cached(task)], None).expect("cached");

  let db = checkout.db();
  let known = recorded::task_of(&db, task).expect("read").expect("the cached task");
  assert_eq!(known.key().to_string(), "KON-7");
  assert!(recorded::unrecorded_runs(&db).expect("runs").is_empty());
  let listed = checkout
    .call("list_runs", &json!({"project": "Kontur"}))
    .expect("listed");
  assert_eq!(listed["runs"][0]["id"], run.to_string().as_str());
  assert_eq!(listed["runs"][0]["task"]["key"], "KON-7");
}

#[test]
fn team_mcp_a_team_task_with_no_run_here_is_not_in_the_hold_queue() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let mut db = checkout.db();
  let link = db.sharing().expect("link").expect("linked");
  let id = TaskId::generate();
  db.record_team_task(
    TeamTaskRef {
      server: link.server,
      project: link.project,
      task: id,
    },
    TaskRevision::try_from(1).expect("revision"),
  )
  .expect("identity");
  db.cache_shared_tasks(&[cached(id)], None).expect("cached");
  assert!(db.task(id).expect("read").is_some(), "the stream's copy is cached");

  assert!(
    recorded::task_of(&db, id).expect("read").is_none(),
    "a copy is no record of this computer"
  );
  assert!(recorded::all(&db).expect("all").is_empty());
  let held = held_work(&db, None).expect("held");
  assert!(
    held.waiting.is_empty(),
    "a team task never run here does not wait here: {held:?}"
  );
}

#[test]
fn team_mcp_tools_on_a_run_with_no_inputs_and_no_row_refuse_it_as_unrecorded() {
  let (checkout, _server, _task, run) = legacy();
  let named = json!({"project": "Kontur", "run": run.to_string()});
  let message = json!({"project": "Kontur", "run": run.to_string(), "text": "Go on."});

  for (tool, args) in [
    ("get_run", &named),
    ("get_plan", &named),
    ("remove_worktree", &named),
    ("send_message", &message),
  ] {
    assert_eq!(checkout.refused(tool, args), "team_task_unrecorded", "{tool}");
  }
}

#[test]
fn team_mcp_an_unreadable_run_of_an_unrecorded_team_task_is_listed_with_its_reason() {
  let (checkout, _server, _task, run) = legacy();
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute("UPDATE runs SET snapshot = 'not json' WHERE id = ?1", [run.to_string()])
    .expect("the run is unreadable");

  let listed = checkout
    .call("list_runs", &json!({"project": "Kontur"}))
    .expect("listed");
  let runs = listed["runs"].as_array().expect("runs");
  assert_eq!(runs.len(), 1, "{listed}");
  assert_eq!(runs[0]["id"], run.to_string().as_str());
  assert_eq!(runs[0]["unrecorded"], true);
  assert_eq!(runs[0]["status"], json!(null));
  assert_eq!(runs[0]["task"], json!(null));
  assert_eq!(runs[0]["detail"], "runs.unreadable.title", "{listed}");
}

#[test]
fn team_mcp_a_cached_team_task_with_no_run_opens_its_page_from_the_server_read() {
  let (checkout, _server, id) = cached_without_run();
  let read = checkout
    .db()
    .task(id)
    .expect("read")
    .expect("the stream's copy is cached");
  assert!(
    checkout.db().shared_task_info(id).expect("info").is_some(),
    "the cached row is a shared one"
  );

  // The page is the server's task, whether or not a copy is cached; it has no run, and is not the copy.
  let detail = TaskDetail::of_team(checkout.root(), (read, None), &Programs::processes()).expect("the page opens");
  assert_eq!(detail.task.id(), id);
  assert!(detail.run.is_none() && detail.history.is_empty());
  // The page without a server read opens from the cached copy; the project's records still go by what this computer
  // recorded, which is nothing, and do not define the task.
  let opened = TaskDetail::load_in(checkout.root(), id).expect("the page opens from the cached copy");
  assert_eq!(opened.task.id(), id);
  assert!(recorded::task_of(&checkout.db(), id).expect("read").is_none());
}

#[test]
fn team_mcp_a_cached_team_task_whose_run_kept_no_inputs_still_opens_its_page() {
  let (checkout, _server, task, _run) = stream_era();
  let read = checkout.db().task(task).expect("read").expect("cached");

  let detail = TaskDetail::of_team(checkout.root(), (read, None), &Programs::processes()).expect("the page opens");
  assert_eq!(detail.task.id(), task);
  assert!(detail.run.is_some(), "its run here is shown");
}
