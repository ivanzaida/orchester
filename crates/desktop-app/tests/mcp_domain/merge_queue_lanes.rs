//! The merge queue's lanes (ORC-406) as `get_run` and `list_runs` tell them: a run's `delivery` names its lane, the
//! base branch it delivers into, and its place is counted within that lane.

use orchester_core::{merge_queue::QueueState, store::RunStore};
use orchester_store::ProjectDb;
use serde_json::json;

use super::Served;

#[test]
fn domain_merge_queue_lanes_get_run_and_list_runs_name_the_lane_of_a_runs_delivery() {
  let (mut served, seeded) = Served::seeded("delivery");
  let db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let waiting = db
    .merge_queue()
    .expect("the queue")
    .into_iter()
    .find(|entry| entry.state == QueueState::WaitingForApproval)
    .expect("a run waiting for approval");
  let run = db.run(waiting.run).expect("read").expect("its run");

  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": run.id().to_string() }));
  assert_eq!(shown["delivery"]["lane"], "main", "{shown}");
  assert_eq!(shown["delivery"]["state"], "waiting_for_approval", "{shown}");
  assert_eq!(shown["delivery"]["place"], 1, "first among the runs of main's lane: {shown}");

  let listed = served.ok("list_runs", &json!({ "project": "Kontur" }));
  let row = listed["runs"]
    .as_array()
    .expect("runs")
    .iter()
    .find(|row| row["id"] == run.id().to_string())
    .unwrap_or_else(|| panic!("the run is listed: {listed}"));
  assert_eq!(row["delivery"]["lane"], "main", "{row}");
}
