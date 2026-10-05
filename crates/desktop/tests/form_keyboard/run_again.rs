//! Run again (ORC-16), headlessly: a failed task offers *Run again* beside *Remove worktree*; its
//! confirmation says what the next run uses and starts it on the real runner and
//! `orchester-fake-codex`, keeping the run before with its worktree unless the option removes it;
//! and while the task cannot run again the action stays unavailable and the line under the notice
//! says why.

use std::{fs, path::Path, process::Command};

use orchester_core::{
  connection::stream::AgentEvent,
  id::{IdempotencyKey, TaskId},
  resource::{ModelId, Pipeline, Slug, VersionNumber},
  run::{Run, RunSnapshot, StepFailure, TokenUsage},
  store::{ActivityStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::{Desktop, expected},
  task_run::{create_and_run, has, init_repository, latest_run, programs, seed},
  unreadable_run::{at, delivery, name, started},
};

fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

fn git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}: {}",
    output.status,
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn worktree_of(repo: &Path, run: &Run) -> std::path::PathBuf {
  repo.join(".orchester").join("worktrees").join(run.id().to_string())
}

/// A task whose one step fails on the fake Codex, its task page showing the failed run.
fn failed_run() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Report failure.", "1400000");
  desktop.wait_until("the failed run with Run again", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().can_run_again())
      && has(desktop, "task-run-again")
      && has(desktop, "task-remove-worktree")
  });
  desktop
}

/// The task page again, which reads what the run left anew.
fn reopen_task(desktop: &mut Desktop) {
  desktop.click("task-breadcrumb");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-run-again");
}

#[test]
fn run_again_on_a_failed_task_confirms_and_starts_its_next_run() {
  let mut desktop = failed_run();
  let first = latest_run(desktop.repo()).expect("the failed run");
  assert!(shows(&mut desktop, "task-run-again", "Run again"));
  desktop.click("task-run-again");
  desktop.wait_for("run-again-dialog");
  assert!(shows(&mut desktop, "run-again-dialog", "Start run #2 of KON-1?"));
  assert!(shows(
    &mut desktop,
    "run-again-dialog",
    "Run #2 starts from the first step in a new worktree, on a new branch from main as it is now."
  ));
  let head = git(desktop.repo(), &["rev-parse", "HEAD"]);
  for text in [
    "Delivery v1 · latest published",
    "Run #1 used",
    "Implement · step 1 of 1",
    &format!("main at {}", &head[..7]),
    "orchester/kon-1-run-2",
    "A new folder in .orchester/worktrees",
    "1.4M tokens · 30 min · from the task",
  ] {
    assert!(shows(&mut desktop, "run-again-properties", text), "{text}");
  }
  assert!(shows(&mut desktop, "run-again-remove", "Remove the worktree of run #1"));
  assert!(shows(&mut desktop, "run-again-remove", "No uncommitted changes in it."));
  assert_eq!(
    desktop.stops(4, false),
    expected(&[
      Some("run-again-remove"),
      Some("run-again-cancel"),
      Some("run-again-confirm"),
      Some("run-again-remove"),
    ])
  );
  // Cancel starts nothing.
  desktop.click("run-again-cancel");
  assert!(!has(&mut desktop, "run-again-dialog"));
  assert_eq!(latest_run(desktop.repo()).expect("run").id(), first.id());

  desktop.click("task-run-again");
  desktop.wait_for("run-again-dialog");
  desktop.tab_to("run-again-confirm", 4);
  desktop.enter();
  desktop.wait_until("run #2 on the page", |desktop| {
    !has(desktop, "run-again-dialog") && latest_run(desktop.repo()).is_some_and(|run| run.number().get() == 2)
  });
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let runs = db.runs_of_task(first.task()).expect("runs");
  assert_eq!(runs.len(), 2, "one run per confirmation");
  let next = latest_run(desktop.repo()).expect("run #2");
  assert_eq!(next.branch().as_str(), "orchester/kon-1-run-2");
  assert!(
    worktree_of(desktop.repo(), &first).is_dir(),
    "run #1 keeps its worktree"
  );
  assert_eq!(
    db.run(first.id()).expect("read"),
    Some(first),
    "run #1 keeps its record"
  );
  desktop.wait_until("run #2 on the page", |desktop| {
    holds(desktop, "task-page", "Run #2") || holds(desktop, "task-page", "Run 2")
  });
}

#[test]
fn run_again_with_the_option_removes_the_worktree_of_the_run_before() {
  let mut desktop = failed_run();
  let first = latest_run(desktop.repo()).expect("the failed run");
  let old = worktree_of(desktop.repo(), &first);
  fs::write(old.join("notes.txt"), "not committed\n").expect("an uncommitted file");
  reopen_task(&mut desktop);
  desktop.click("task-run-again");
  desktop.wait_until("the option naming the file", |desktop| {
    holds(
      desktop,
      "run-again-remove",
      "1 file not committed would be discarded: notes.txt",
    )
  });
  desktop.click("run-again-remove");
  desktop.click("run-again-confirm");
  desktop.wait_until("run #2", |desktop| {
    !has(desktop, "run-again-dialog") && latest_run(desktop.repo()).is_some_and(|run| run.number().get() == 2)
  });
  assert!(
    !old.exists(),
    "the worktree of run #1 was removed with its uncommitted file"
  );
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert!(db.run(first.id()).expect("read").is_some(), "run #1 keeps its record");
}

/// KON-1, queued on `pipeline`.
fn queued_task(db: &mut ProjectDb, pipeline: orchester_core::id::ResourceId) -> TaskId {
  let details = TaskDetails {
    title: name("Add repository switcher"),
    brief: "Switch repositories.".into(),
    acceptance_criteria: vec!["It switches".into()],
    pipeline: Some(pipeline),
    limits: Some(
      TaskLimits::new(
        TokenBudget::new(1_400_000).expect("above zero"),
        std::time::Duration::from_secs(1_800),
      )
      .expect("limits"),
    ),
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
    .expect("task")[0]
    .clone();
  task.queue(VersionNumber::FIRST).expect("a complete task");
  db.save_task(&task).expect("queue");
  task.id()
}

/// KON-1, queued on `pipeline`, with run #1 of `snapshot`, which failed.
fn failed_task(db: &mut ProjectDb, snapshot: &RunSnapshot, pipeline: orchester_core::id::ResourceId) -> Run {
  let task = queued_task(db, pipeline);
  let run = started(db, (task, snapshot), 1, 10);
  fail(db, run)
}

fn fail(db: &mut ProjectDb, mut run: Run) -> Run {
  let execution = run.current_attempt().expect("an attempt").executions()[0].id();
  let events = run.start_step(execution, at(20)).expect("start the step");
  db.append_events(&run, &events).expect("append");
  let events = run
    .fail_step(execution, StepFailure::ReportedFailed, at(21))
    .expect("fail the step");
  db.append_events(&run, &events).expect("append");
  run
}

fn pipeline_of(snapshot: &RunSnapshot) -> orchester_core::id::ResourceId {
  snapshot.pipeline().resource
}

/// The task page of KON-1 in a project `seed` filled, and why it cannot run again.
fn unavailable(seed: impl FnOnce(&Path), reason: &str) {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_until("why Run again is unavailable", |desktop| {
    holds(desktop, "task-run-again-reason", reason)
  });
  assert!(shows(&mut desktop, "task-run-again", "Run again"));
  desktop.click("task-run-again");
  assert!(
    !has(&mut desktop, "run-again-dialog"),
    "an unavailable Run again opens nothing"
  );
}

#[test]
fn run_again_is_unavailable_while_another_run_of_the_task_is_active() {
  unavailable(
    |repo| {
      let mut db = ProjectDb::open(repo).expect("open");
      let snapshot = delivery(&mut db);
      // Run #1 has not ended, while run #2 failed: a task whose record holds two runs at once, as
      // a request made from a page that still showed an earlier run would leave it.
      let task = queued_task(&mut db, pipeline_of(&snapshot));
      started(&mut db, (task, &snapshot), 1, 10);
      let second = started(&mut db, (task, &snapshot), 2, 30);
      fail(&mut db, second);
    },
    "Run again is unavailable: run #1 of this task is active.",
  );
}

#[test]
fn run_again_is_unavailable_once_the_pipeline_is_no_longer_published() {
  unavailable(
    |repo| {
      let mut db = ProjectDb::open(repo).expect("open");
      let snapshot = delivery(&mut db);
      let body = db
        .latest_version::<Pipeline>(pipeline_of(&snapshot))
        .expect("read")
        .expect("published")
        .body()
        .clone();
      let nightly = db
        .create_resource(Slug::new("nightly").expect("slug"), name("Nightly"), &body, at(4))
        .expect("draft pipeline")
        .id();
      failed_task(&mut db, &snapshot, nightly);
    },
    "Run again is unavailable: Nightly has no published version any more.",
  );
}

#[test]
fn run_again_is_unavailable_while_the_daily_budget_is_used() {
  unavailable(
    |repo| {
      let mut db = ProjectDb::open(repo).expect("open");
      let snapshot = delivery(&mut db);
      let run = failed_task(&mut db, &snapshot, pipeline_of(&snapshot));
      let execution = run.attempts()[0].executions()[0].id();
      let model = ModelId::new("gpt-5.5").expect("model");
      let invocation = db
        .start_invocation(run.id(), execution, &model, Timestamp::now())
        .expect("invocation");
      let usage = TokenUsage {
        input: 20_000_000,
        cached_input: 0,
        output: 0,
      };
      db.append_activity(&invocation, &[(Timestamp::now(), AgentEvent::TurnCompleted { usage })])
        .expect("today's usage");
    },
    "Run again is unavailable: the daily token budget is used (20M of 20M today).",
  );
}
