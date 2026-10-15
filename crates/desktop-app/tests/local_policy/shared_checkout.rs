use std::fs;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::local::LocalIntegrationStore,
  name::Name,
  run::{BranchName, NewRun},
  store::{MergeStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  client::{Client, ok, value},
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn args(page: &Page) -> serde_json::Value {
  serde_json::json!({"project": ProjectDb::open(page.root()).expect("db").project().id().to_string(),
    "run": page.run().to_string()})
}

fn next_run(fixture: &mut Fixture) -> RunId {
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let previous = db.run(fixture.run).expect("run").expect("exists");
  let task = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: Name::new("Separate integration").expect("name"),
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: None,
        limits: TaskLimits::NONE,
      }],
      Timestamp::from_unix_millis(20),
    )
    .expect("task")
    .remove(0);
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: task.id(),
        snapshot: previous.snapshot().clone(),
        branch: BranchName::new("next-result").expect("branch"),
        base: git(fixture.root(), &["rev-parse", "HEAD"]).parse().expect("base"),
        base_branch: Some(BranchName::new("main").expect("branch")),
        budget: None,
        max_duration: None,
      },
      Timestamp::from_unix_millis(21),
    )
    .expect("run");
  let events = run.start(Timestamp::from_unix_millis(22)).expect("start");
  db.append_events(&run, &events).expect("events");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .start_step(execution, Timestamp::from_unix_millis(23))
    .expect("step");
  db.append_events(&run, &events).expect("events");
  let events = run
    .complete_step(execution, Timestamp::from_unix_millis(24))
    .expect("pass");
  db.append_events(&run, &events).expect("events");
  fixture.run = run.id();
  fixture.task_root = fixture.root().join(".orchester/worktrees").join(run.id().to_string());
  git(
    fixture.root(),
    &[
      "worktree",
      "add",
      "-b",
      "next-result",
      fixture.task_root.to_str().expect("path"),
    ],
  );
  fs::write(fixture.task_root.join("next.txt"), "separate actual result\n").expect("edit");
  git(&fixture.task_root, &["add", "next.txt"]);
  git(&fixture.task_root, &["commit", "-m", "separate result"]);
  previous.id()
}

#[test]
fn local_policy_other_run_pending_review_blocks_legacy_until_its_owned_guard_drops() {
  let mut page = Page::new(Fixture::new(false));
  page.ready("local-review-preserve");
  let project = ProjectDb::open(page.root()).expect("db").project().id();
  let jobs = page.window.launch.services().expect("services").jobs.of(project);
  let other = RunId::generate();
  let review = jobs.of(other).local_decision().expect("reserve");
  let second = jobs.of(other).local_decision().expect("second real decision");
  let before = git(page.root(), &["rev-parse", "HEAD"]);
  let client = Client::new(&mut page, false);
  let arguments = args(&page);
  assert_eq!(
    value(&client.call(&mut page, "merge_run", &arguments))["code"],
    "needs_decision"
  );
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), before);
  drop(review);
  assert!(
    jobs.state().local_decision_pending(),
    "one real review still owns checkout"
  );
  drop(second);
  assert!(!jobs.state().local_decision_pending());
  ok(&client.call(&mut page, "merge_run", &arguments));
  assert!(
    ProjectDb::open(page.root())
      .expect("db")
      .run_merge(page.run())
      .expect("record")
      .is_some()
  );
}

#[test]
fn local_policy_other_run_retained_recovery_blocks_legacy_but_verified_cleanup_releases_checkout() {
  for cleanup in [false, true] {
    let mut fixture = Fixture::new(false);
    fs::write(fixture.root().join("local.txt"), "personal original\n").expect("edit");
    let mut session = fixture.open();
    session.preserve("Private historical work").expect("preserve");
    session.cancel_preservation().expect("actual restoration");
    if cleanup {
      session.remove_preservation().expect("verified cleanup");
    }
    drop(session);
    assert_eq!(
      fs::read(fixture.root().join("local.txt")).expect("original"),
      b"personal original\n"
    );
    git(fixture.root(), &["commit", "-am", "retain personal result"]);
    let historical = next_run(&mut fixture);
    let mut page = Page::new(fixture);
    page.ready("local-review-preserve");
    let before = git(page.root(), &["rev-parse", "HEAD"]);
    let db = ProjectDb::open(page.root()).expect("db");
    let saved = db.local_operation(historical).expect("history").expect("exists");
    assert_eq!(db.local_operations().expect("typed listing"), vec![saved.clone()]);
    let client = Client::new(&mut page, false);
    let arguments = args(&page);
    let result = client.call(&mut page, "merge_run", &arguments);
    if cleanup {
      ok(&result);
      assert!(db.run_merge(page.run()).expect("actual outcome").is_some());
    } else {
      assert_eq!(value(&result)["code"], "needs_decision");
      assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), before);
      assert!(db.run_merge(page.run()).expect("no forged outcome").is_none());
    }
    assert_eq!(db.local_operation(historical).expect("history"), Some(saved));
  }
}
