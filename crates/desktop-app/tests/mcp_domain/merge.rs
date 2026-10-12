//! `merge_run` takes only the task's latest run once it is ready for review.

use orchester_core::{
  id::IdempotencyKey,
  run::{BranchName, NewRun},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::Served;

#[test]
fn domain_merge_run_takes_only_the_latest_run_ready_for_review() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Show the signed-in account")
    .expect("the seeded task ready for review");
  let ready = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .find_map(Result::ok)
    .expect("its run");
  let merge = |run: &orchester_core::run::Run| json!({ "project": "Kontur", "run": run.id().to_string() });
  assert_eq!(served.refused("merge_run", &merge(&ready))["code"], "person_required");
  let later = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: task.id(),
        snapshot: ready.snapshot().clone(),
        branch: BranchName::for_next_run(task.key(), 1),
        base: ready.base().clone(),
        base_branch: ready.base_branch().cloned(),
        budget: ready.budget(),
        max_duration: ready.max_duration(),
      },
      Timestamp::now(),
    )
    .expect("a later run");
  assert_eq!(served.refused("merge_run", &merge(&later))["code"], "run_not_ready");
  assert_eq!(served.refused("merge_run", &merge(&ready))["code"], "not_latest_run");
}
