//! A run whose work was merged outside Orchester, with plain git as a pull request pulled back would: `list_runs` and
//! `get_run` read it merged, as the task's Summary does, and `merge_run` answers that it was merged already instead
//! of asking for a person. Only the branch's whole work in its base branch counts: part of it still waits.

use orchester_core::{
  run::Run,
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Seeded, Served, live::git};

/// The seeded run ready for review.
fn ready_run(seeded: &Seeded) -> Run {
  let db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Show the signed-in account")
    .expect("the seeded task ready for review");
  db.runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .find_map(Result::ok)
    .expect("its run")
}

fn listed(served: &mut Served, run: &Run) -> Value {
  let runs = served.ok("list_runs", &json!({ "project": "Kontur" }));
  runs["runs"]
    .as_array()
    .expect("runs")
    .iter()
    .find(|listed| listed["id"] == run.id().to_string())
    .expect("the run is listed")
    .clone()
}

fn in_run(run: &Run) -> Value {
  json!({ "project": "Kontur", "run": run.id().to_string() })
}

#[test]
fn domain_run_merged_outside_reads_merged_and_merge_run_answers_already_merged() {
  let (mut served, seeded) = Served::seeded("tasks");
  let run = ready_run(&seeded);
  let into = run.base_branch().expect("a base branch").as_str().to_owned();
  assert_eq!(listed(&mut served, &run)["phase"], "succeeded");
  git(&seeded.repo, &["merge", "--no-edit", run.branch().as_str()]);
  let merged = listed(&mut served, &run);
  assert_eq!(merged["phase"], "merged", "{merged}");
  assert!(
    merged["detail"].as_str().expect("detail").contains("without Orchester"),
    "{merged}"
  );
  let read = served.ok("get_run", &in_run(&run));
  assert_eq!(read["merge"]["state"], "merged", "{read}");
  assert_eq!(read["merge"]["by"], "outside");
  assert_eq!(read["merge"]["into"], into);
  let refused = served.refused("merge_run", &in_run(&run));
  assert_eq!(refused["code"], "already_merged", "{refused}");
}

#[test]
fn domain_run_merged_only_in_part_still_waits_for_the_person() {
  let (mut served, seeded) = Served::seeded("tasks");
  let run = ready_run(&seeded);
  let branch = run.branch().as_str();
  let delivered = git(&seeded.repo, &["rev-parse", branch]).trim().to_owned();
  let tree = git(&seeded.repo, &["rev-parse", &format!("{branch}^{{tree}}")])
    .trim()
    .to_owned();
  let more = git(
    &seeded.repo,
    &["commit-tree", &tree, "-p", &delivered, "-m", "more work"],
  )
  .trim()
  .to_owned();
  git(&seeded.repo, &["update-ref", &format!("refs/heads/{branch}"), &more]);
  git(&seeded.repo, &["merge", "--no-edit", &delivered]);
  let listed = listed(&mut served, &run);
  assert_eq!(listed["phase"], "succeeded", "{listed}");
  let read = served.ok("get_run", &in_run(&run));
  assert_eq!(read["merge"]["state"], "not_merged", "{read}");
  let refused = served.refused("merge_run", &in_run(&run));
  assert_ne!(refused["code"], "already_merged", "{refused}");
}
