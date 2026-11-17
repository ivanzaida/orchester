//! What the Orchestrator reads of a team task it leads, as it reads it of a local one (`docs/design/local-and-team-
//! parity.md`): where its target stands says whether the task's pipeline is ready here and holds the server's Blocked
//! mark whatever watches the task, the whole project too; `/watching` names what waits in a team task's run; and a
//! kept run end of a task the server holds Blocked says it is left to the person, not that the chat merges it.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  orchestrator::{MergeLeft, Merging, WatchTarget},
  run::WaitReason,
};
use orchester_workspace::orchestrator::{
  auto_merge::merging_in,
  watching::{Going, WaitsFor, WatchReads},
};
use serde_json::json;

use crate::{
  Checkout,
  led_module::{leading, named, stands},
  team_cache::keep_cached_team,
  team_runs::started,
  tools::server,
  watch_ends::{module_watch, refreshed},
};

#[test]
fn team_mcp_where_a_led_module_stands_says_whether_a_team_tasks_pipeline_is_ready() {
  let (server, seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });
  keep_cached_team(&checkout.workspace, checkout.root());
  let target = module_watch(&checkout, server.module);
  leading(&checkout, target);
  let stands_now = |fields| {
    seen.lock().task_fields = fields;
    refreshed(&checkout);
    named(&stands(&checkout))
      .into_iter()
      .map(|(task, standing)| (task.to_owned(), standing.to_owned()))
      .collect::<Vec<_>>()
  };
  let kon7 = |standing: &str| vec![("KON-7".to_owned(), standing.to_owned())];

  // The team's pipeline the task names is one this session read: it can start.
  assert_eq!(stands_now(json!(null)), kon7("ready_to_start"));
  // A task that names no pipeline, or one that is none of the team's here, cannot, as a local task cannot.
  assert_eq!(stands_now(json!({ "pipeline": null })), kon7("no_pipeline"));
  let other = ResourceId::generate().to_string();
  assert_eq!(stands_now(json!({ "pipeline": other })), kon7("pipeline_not_ready"));
}

#[test]
fn team_mcp_a_team_task_led_through_the_whole_project_holds_the_servers_blocked_mark() {
  let team = started();
  leading(&team.checkout, WatchTarget::Project);
  refreshed(&team.checkout);
  assert_eq!(named(&stands(&team.checkout)), [("KON-7", "working")]);

  team.seen.lock().task_fields = json!({ "blocked_reason": "Waits for the vendor" });
  refreshed(&team.checkout);

  assert_eq!(named(&stands(&team.checkout)), [("KON-7", "parked_by_person")]);
}

#[test]
fn team_mcp_a_team_task_that_never_ran_here_is_led_through_the_whole_project() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  leading(&checkout, WatchTarget::Project);

  assert!(
    refreshed(&checkout).polling,
    "the project's tasks are read from the server"
  );

  assert_eq!(named(&stands(&checkout)), [("KON-7", "ready_to_start")]);
}

#[test]
fn team_mcp_a_kept_team_run_end_is_left_to_the_person_while_the_server_holds_its_task_blocked() {
  let team = started();
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let target = module_watch(&team.checkout, team.server.module);
  leading(&team.checkout, target);
  refreshed(&team.checkout);
  assert_eq!(
    merging_in(&team.checkout.db(), team.run).expect("rule"),
    Merging::Merges
  );

  team.seen.lock().task_fields = json!({ "blocked_reason": "Waiting for the staging key" });
  refreshed(&team.checkout);

  assert_eq!(
    merging_in(&team.checkout.db(), team.run).expect("rule"),
    Merging::Left {
      why: MergeLeft::MarkedBlocked {
        reason: "Waiting for the staging key".to_owned()
      }
    }
  );
}

#[test]
fn team_mcp_watching_names_what_waits_in_a_team_tasks_run() {
  let team = started();
  team.step_into(|run, execution, now| run.wait_step(execution, WaitReason::Message, now).expect("waits"));
  let target = module_watch(&team.checkout, team.server.module);
  let chat = leading(&team.checkout, target);
  refreshed(&team.checkout);

  let reads = WatchReads::read(&team.checkout.db(), team.checkout.root()).expect("reads");
  let listing = reads.watching(chat);

  let tasks = &listing.subscriptions[0].tasks;
  assert_eq!(tasks.len(), 1, "{listing:?}");
  assert_eq!(&*tasks[0].key, "KON-7");
  assert_eq!(tasks[0].going, Going::Waits(WaitsFor::Message), "{listing:?}");
  assert_eq!(tasks[0].run.map(|run| run.get()), Some(1));
}
