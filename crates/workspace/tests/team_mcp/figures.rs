//! `get_delivery` of a team project (`docs/plan/orchestrator-autonomy/OA-36-…`): the figures of a local project, built
//! from the tasks and modules the team server holds and this computer's runs of them, with no cumulative flow, as the
//! server keeps no history of a task's workflow status; a module is named among the server's.

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use serde_json::{Value, json};

use crate::{
  Checkout, git,
  team_runs::{Started, started},
  team_server::MODULE_BRANCH,
  tools::server,
};

fn forecasts(delivery: &Value) -> Vec<(String, u64)> {
  delivery["forecasts"]
    .as_array()
    .expect("forecasts")
    .iter()
    .map(|forecast| {
      let work = &forecast["work"];
      let named = work["name"].as_str().or_else(|| work["kind"].as_str()).expect("named");
      (named.to_owned(), forecast["remaining"].as_u64().expect("remaining"))
    })
    .collect()
}

#[test]
fn team_mcp_figures_of_a_team_project_are_its_servers_tasks() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);

  let delivery = checkout
    .call("get_delivery", &json!({"project": "Kontur"}))
    .expect("figures");
  assert_eq!(delivery["team"], true);
  assert_eq!(
    delivery["cumulative_flow"],
    json!({"unavailable": "team_no_status_history"})
  );
  assert_eq!(
    forecasts(&delivery),
    [("queued_tasks".to_owned(), 1), ("Webhooks".to_owned(), 1)],
    "KON-7 is open, in Webhooks"
  );

  let webhooks = json!({"project": "Kontur", "module": "webhooks"});
  let narrowed = checkout.call("get_delivery", &webhooks).expect("figures");
  assert_eq!(forecasts(&narrowed).len(), 2, "{narrowed}");
  let unknown = json!({"project": "Kontur", "module": "nothing"});
  assert_eq!(checkout.refused("get_delivery", &unknown), "module_not_found");
}

#[test]
fn team_mcp_figures_count_the_team_tasks_merged_on_this_computer() {
  let Started { checkout, run, .. } = started();
  let mut db = checkout.db();
  let mut ended = db.run(run).expect("read").expect("the run");
  let events = ended.cancel(Timestamp::now()).expect("canceled");
  db.append_events(&ended, &events).expect("recorded");
  db.save_run_merge(&RunMerge {
    run,
    into: BranchName::new(MODULE_BRANCH).expect("a branch"),
    revision: Revision::new(&git(checkout.root(), &["rev-parse", MODULE_BRANCH])).expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  })
  .expect("the merge");
  drop(db);

  let delivery = checkout
    .call("get_delivery", &json!({"project": "Kontur", "days": 7}))
    .expect("figures");
  let merged: u64 = delivery["throughput"]["bars"]
    .as_array()
    .expect("bars")
    .iter()
    .map(|bar| bar["tasks"].as_u64().expect("a count"))
    .sum();
  assert_eq!(merged, 1, "{delivery}");
  assert_eq!(delivery["cycle_time"]["tasks"][0]["key"], "KON-7");
}
