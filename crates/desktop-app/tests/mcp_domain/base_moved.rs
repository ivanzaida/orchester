//! A notice that a run's base branch moved (ORC-54) is reported by `get_run` and `get_conversation` as kind
//! `base_moved`, with the revision, what landed, the files both sides changed and where it stands, so the
//! Orchestrator chat can see and report it.

use orchester_core::{
  base_move::{BaseNotice, LandedCommit, LandedTask, NoticeDelivery},
  id::NoticeId,
  revision::Revision,
  run::BranchName,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::Served;

#[test]
fn domain_get_run_and_get_conversation_report_the_base_moved_notice() {
  let (mut served, seeded) = Served::seeded("decisions");
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let task = db.tasks().expect("tasks").remove(0);
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let revision = Revision::new(&"c".repeat(40)).expect("revision");
  db.queue_base_notice(&BaseNotice {
    id: NoticeId::generate(),
    run: run.id(),
    task: task.id(),
    branch: BranchName::new("main").expect("branch"),
    revision: revision.clone(),
    commits: vec![LandedCommit {
      revision: revision.clone(),
      subject: "Merge KON-9 run #4".to_owned(),
    }],
    tasks: vec![LandedTask {
      key: "KON-9".to_owned(),
      title: "Pull the people's change".to_owned(),
    }],
    overlapping: vec!["src/login.rs".to_owned()],
    noticed_at: Timestamp::now(),
    deliver_after: Timestamp::now(),
    delivery: NoticeDelivery::Pending,
  })
  .expect("queued");

  let number = run.number().get().to_string();
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": number }));
  let notice = &shown["base_moved"][0];
  assert_eq!(notice["kind"], "base_moved", "{shown}");
  assert_eq!(notice["branch"], "main");
  assert_eq!(notice["revision"], revision.as_str());
  assert_eq!(notice["tasks"][0]["key"], "KON-9");
  assert_eq!(notice["commits"][0]["subject"], "Merge KON-9 run #4");
  assert_eq!(notice["overlapping_files"], json!(["src/login.rs"]));
  assert!(
    notice["state"].as_str().is_some_and(|state| state != "delivered"),
    "{notice}"
  );

  let read = served.ok(
    "get_conversation",
    &json!({ "project": "Kontur", "task": task.key().to_string(), "limit": 200 }),
  );
  let entries = read["entries"].as_array().expect("entries");
  let entry = entries
    .iter()
    .find(|entry| entry["kind"] == "base_moved")
    .unwrap_or_else(|| panic!("the notice is an entry: {read}"));
  assert_eq!(entry["author"]["kind"], "orchester");
  assert_eq!(entry["revision"], revision.as_str());
  assert_eq!(entry["delivered_to"], serde_json::Value::Null);
}

/// A move that overlapped none of the run's changes, which the runner merged itself before a step started (ORC-151):
/// `get_run` says so, with that step and the merge commit, and names no agent it was delivered to.
#[test]
fn domain_get_run_reports_a_base_move_the_runner_merged() {
  let (mut served, seeded) = Served::seeded("decisions");
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let task = db.tasks().expect("tasks").remove(0);
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let execution = run
    .current_attempt()
    .and_then(|attempt| attempt.executions().first())
    .expect("an execution");
  let step = run
    .snapshot()
    .body()
    .step(execution.step())
    .expect("its step")
    .name
    .as_str()
    .to_owned();
  let revision = Revision::new(&"c".repeat(40)).expect("revision");
  let merge = Revision::new(&"d".repeat(40)).expect("merge commit");
  let notice = BaseNotice {
    id: NoticeId::generate(),
    run: run.id(),
    task: task.id(),
    branch: BranchName::new("main").expect("branch"),
    revision: revision.clone(),
    commits: vec![LandedCommit {
      revision: revision.clone(),
      subject: "Merge KON-9 run #4".to_owned(),
    }],
    tasks: Vec::new(),
    overlapping: Vec::new(),
    noticed_at: Timestamp::now(),
    deliver_after: Timestamp::now(),
    delivery: NoticeDelivery::Pending,
  };
  db.queue_base_notice(&notice).expect("queued");
  assert!(
    db.merge_base_notice(&notice, (execution.id(), &merge), Timestamp::now())
      .expect("merged")
  );
  assert!(
    db.pending_base_notice(run.id()).expect("read").is_none(),
    "no longer pending"
  );

  let number = run.number().get().to_string();
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": number }));
  let notice = &shown["base_moved"][0];
  assert_eq!(notice["state"], "merged_by_runner", "{shown}");
  assert_eq!(notice["delivered_to"], serde_json::Value::Null);
  assert_eq!(notice["merged_by_runner"]["before_step"], step.as_str());
  assert_eq!(notice["merged_by_runner"]["merge_commit"], merge.as_str());
  assert_eq!(notice["revision"], revision.as_str());
}
